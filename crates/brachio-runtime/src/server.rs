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
    accepted, await_outcome, read_message, write_message, Action, Request, Response,
    ResponseResult, StatusInfo, IO_TIMEOUT, MAX_MESSAGE, MAX_RESPONSE,
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
        .map_or_else(|| journal::default_directory(pipe), Ok)
        .and_then(|directory| Journal::open(&directory));
    let journal = match journal {
        Ok(journal) => Some(journal),
        Err(error) => {
            eprintln!("running without a journal: {error}");
            None
        }
    };
    run(listener, Arc::new(Runtime::new(journal))).await
}

/// Accepts connections. Reaping and journal writes run on a blocking thread, one pass at a time,
/// so a slow disk delays only the next pass and never the accept loop.
async fn run(
    listener: interprocess::local_socket::tokio::Listener,
    runtime: Arc<Runtime>,
) -> Result<()> {
    let mut check = tokio::time::interval(Duration::from_secs(1));
    check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut maintenance: Option<tokio::task::JoinHandle<()>> = None;
    loop {
        let stream = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(stream) => stream,
                // One failed accept must not end the process that keeps every session alive.
                Err(error) => {
                    eprintln!("runtime could not accept a connection: {error}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            },
            _ = check.tick(), if maintenance.is_none() => {
                let runtime = Arc::clone(&runtime);
                maintenance = Some(tokio::task::spawn_blocking(move || runtime.maintain()));
                continue;
            }
            _ = async { (&mut maintenance.as_mut().expect("guarded")).await }, if maintenance.is_some() => {
                maintenance = None;
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
            await_outcome(
                request.action.changes_state(),
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

#[cfg(test)]
mod stalled_journal_tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use brachio_runtime::client::Client;
    use brachio_runtime::transport;
    use brachio_runtime::wire::{Action, ResponseResult};

    use super::run;
    use crate::journal::Journal;
    use crate::runtime::Runtime;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_stalled_journal_write_does_not_stop_new_requests() {
        let directory = std::env::temp_dir().join(format!("brachio-stall-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let mut journal = Journal::open(&directory).expect("open journal");
        let (entered_sender, entered) = mpsc::channel();
        let (release, release_receiver) = mpsc::channel::<()>();
        let entered_sender = Mutex::new(entered_sender);
        let release_receiver = Mutex::new(release_receiver);
        journal.before_save = Some(Box::new(move || {
            let _ = entered_sender.lock().expect("entered").send(());
            let _ = release_receiver.lock().expect("release").recv();
        }));
        let runtime = Arc::new(Runtime::new(Some(journal)));
        let pipe = format!("stall-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let launch = Arc::clone(&runtime);
        let key = "task/stalled".to_owned();
        let cwd = std::env::temp_dir();
        tokio::task::spawn_blocking(move || {
            launch.execute(brachio_runtime::wire::Request {
                version: brachio_runtime::protocol::CURRENT,
                action: Action::Ensure { key, cwd },
            })
        })
        .await
        .expect("join")
        .expect("launch");
        let listener = transport::listen(&pipe).expect("listen");
        let server = tokio::spawn(run(listener, Arc::clone(&runtime)));

        tokio::task::spawn_blocking(move || entered.recv_timeout(Duration::from_secs(30)))
            .await
            .expect("join")
            .expect("journal write never started");
        let client = Client::new(&pipe);
        let reply = tokio::time::timeout(Duration::from_secs(10), client.call(Action::Hello))
            .await
            .expect("hello answered while the journal write is stalled")
            .expect("hello");
        assert!(matches!(reply.result, ResponseResult::Hello { .. }));

        drop(release);
        server.abort();
        let _ = runtime.execute(brachio_runtime::wire::Request {
            version: brachio_runtime::protocol::CURRENT,
            action: Action::Stop {
                key: "task/stalled".to_owned(),
            },
        });
        let _ = std::fs::remove_dir_all(directory);
    }
}
