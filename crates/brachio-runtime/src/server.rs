//! Accepts connections and answers requests until the runtime has nothing left to serve.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use brachio_runtime::endpoint::{self, Liveness};
use brachio_runtime::protocol;
use brachio_runtime::transport;
use brachio_runtime::wire::{
    accepted, read_message, write_message, Action, Request, Response, ResponseResult, StatusInfo,
    IO_TIMEOUT, MAX_MESSAGE, MAX_RESPONSE,
};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::Stream;
use tokio::io::{AsyncReadExt, BufReader};

use crate::journal::{self, Journal};
use crate::runtime::Runtime;
use crate::session::{get, lock, Session};

pub(crate) async fn serve(pipe: &str, state_dir: Option<PathBuf>) -> Result<()> {
    let listener = match own(pipe).await {
        Ok(listener) => listener,
        Err((code, error)) => {
            eprintln!("cannot own endpoint {pipe}: {error}");
            std::process::exit(code);
        }
    };
    // The journal is read only after this process owns the endpoint, so every record in it
    // belongs to a runtime that is gone.
    let journal = state_dir
        .map_or_else(journal::default_directory, Ok)
        .and_then(|directory| Journal::open(&directory));
    let journal = match journal {
        Ok(journal) => Some(journal),
        Err(error) => {
            eprintln!("running without a journal: {error}");
            None
        }
    };
    let runtime = Arc::new(Runtime::new(journal));
    let mut check = tokio::time::interval(Duration::from_secs(1));
    loop {
        let stream = tokio::select! {
            stream = listener.accept() => stream?,
            _ = check.tick() => {
                runtime.persist();
                if runtime.finished() {
                    return Ok(());
                }
                continue;
            }
        };
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            if let Err(error) = respond(stream, runtime).await {
                let disconnected = error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe);
                if !disconnected {
                    eprintln!("runtime connection ended: {error}");
                }
            }
        });
    }
}

/// Take the endpoint. Another runtime answering proves the endpoint is in use. Only a refused
/// or missing endpoint proves its owner is gone, and only then is the leftover replaced.
pub(crate) async fn own(
    pipe: &str,
) -> Result<interprocess::local_socket::tokio::Listener, (i32, io::Error)> {
    let first = match transport::listen(pipe) {
        Ok(listener) => return Ok(listener),
        Err(error) => error,
    };
    let probe = transport::connect(pipe).await;
    match endpoint::classify(&probe) {
        Liveness::Serving => Err((endpoint::EXIT_OCCUPIED, first)),
        Liveness::Unknown => Err((endpoint::EXIT_UNCERTAIN, first)),
        Liveness::Dead => transport::reclaim(pipe)
            .and_then(|()| transport::listen(pipe))
            .map_err(|error| (endpoint::EXIT_UNCERTAIN, error)),
    }
}

pub(crate) async fn respond(mut stream: Stream, runtime: Arc<Runtime>) -> Result<()> {
    let message = tokio::time::timeout(
        IO_TIMEOUT,
        read_message(&mut BufReader::new(&mut stream), MAX_MESSAGE),
    )
    .await??
    .context("connection closed before request")?;
    let request = serde_json::from_slice::<Request>(&message)
        .map_err(anyhow::Error::from)
        .and_then(accepted);
    let version = request
        .as_ref()
        .map_or(protocol::CURRENT, |request| request.version);
    let result = match request {
        Ok(Request {
            action: Action::Follow { key, after },
            ..
        }) => {
            let session =
                lock(&runtime.sessions).and_then(|sessions| get(&sessions, &key).cloned());
            match session {
                Ok(session) => {
                    return follow(&mut stream, &runtime.instance, version, session, after).await
                }
                Err(error) => Err(error),
            }
        }
        Ok(Request {
            action: Action::FollowFrame { key },
            ..
        }) => {
            let session =
                lock(&runtime.sessions).and_then(|sessions| get(&sessions, &key).cloned());
            match session {
                Ok(session) => {
                    let snapshot = session.snapshot()?;
                    if let ResponseResult::Snapshot { screen, .. } = &snapshot {
                        if let Some(offset) = screen.resume_offset {
                            send_response(&mut stream, &runtime.instance, version, Ok(snapshot))
                                .await?;
                            return follow(
                                &mut stream,
                                &runtime.instance,
                                version,
                                session,
                                offset,
                            )
                            .await;
                        }
                    }
                    Err(anyhow!(
                        "terminal frame is not at a supported replay boundary"
                    ))
                }
                Err(error) => Err(error),
            }
        }
        Ok(request) => {
            tokio::time::timeout(
                IO_TIMEOUT,
                tokio::task::spawn_blocking({
                    let runtime = Arc::clone(&runtime);
                    move || runtime.execute(request)
                }),
            )
            .await??
        }
        Err(error) => Err(error),
    };
    send_response(&mut stream, &runtime.instance, version, result).await
}

pub(crate) async fn send_response(
    stream: &mut Stream,
    instance: &str,
    version: u8,
    result: Result<ResponseResult>,
) -> Result<()> {
    let result = match result {
        Ok(result) => result,
        Err(error) => ResponseResult::Error {
            error: format!("{error:#}"),
        },
    };
    tokio::time::timeout(
        IO_TIMEOUT,
        write_message(
            stream,
            &Response {
                version,
                instance,
                result,
            },
            MAX_RESPONSE,
        ),
    )
    .await?
}

pub(crate) async fn follow(
    stream: &mut Stream,
    instance: &str,
    version: u8,
    mut session: Session,
    mut offset: u64,
) -> Result<()> {
    let mut first = true;
    let mut last_status: Option<StatusInfo> = None;
    loop {
        // Mark updates before reading the journal. A write that arrives after
        // the snapshot remains observable when we begin waiting.
        session.updates.borrow_and_update();
        let result = session.describe(Some(offset));
        let (next, end, exited, status) = match &result {
            Ok(ResponseResult::Success {
                session,
                output: Some(page),
            }) => (page.next, page.end, session.exited, session.status.clone()),
            _ => return send_response(stream, instance, version, result).await,
        };
        if first || next != offset || exited || status != last_status {
            send_response(stream, instance, version, result).await?;
        }
        first = false;
        last_status = status;
        offset = next;
        if offset < end {
            continue;
        }
        if exited {
            return Ok(());
        }
        let mut input = [0];
        tokio::select! {
            changed = session.updates.changed() => { changed.context("terminal updates closed")?; }
            _ = stream.read(&mut input) => return Ok(()),
        }
    }
}
