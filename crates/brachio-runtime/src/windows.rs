//! Local named-pipe control of an independent Windows PTY runtime.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use brachio_pty::{PtyEvent, SessionId, SessionManager, SpawnSpec};
use clap::{Parser, Subcommand};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
use interprocess::os::windows::local_socket::ListenerOptionsExt;
use interprocess::os::windows::security_descriptor::SecurityDescriptor;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;
use widestring::U16CString;

const VERSION: u8 = 1;
const MAX_MESSAGE: u64 = 64 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const CURSOR_QUERY: &[u8] = b"\x1b[6n";

#[derive(Parser)]
struct Args {
    /// Local pipe name for this runtime instance.
    #[arg(long)]
    pipe: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Own PTYs until this process exits.
    Serve,
    /// Send one request to a running instance.
    Send {
        #[command(subcommand)]
        action: Action,
    },
}

#[derive(Subcommand, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Action {
    /// Start one shell for this key, or return the existing one.
    Ensure { key: String, cwd: PathBuf },
    /// Read the shell identity and number of PTY bytes received.
    Inspect { key: String },
    /// Write text to the shell.
    Write { key: String, text: String },
    /// Stop one shell explicitly.
    Stop { key: String },
}

#[derive(Serialize, Deserialize)]
struct Request {
    version: u8,
    action: Action,
}

#[derive(Serialize)]
struct Response<'a> {
    version: u8,
    instance: &'a str,
    #[serde(flatten)]
    result: ResponseResult,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ResponseResult {
    Success { session: SessionInfo },
    Error { error: String },
}

#[derive(Serialize)]
struct SessionInfo {
    session_id: SessionId,
    pid: u32,
    output_bytes: u64,
}

struct Session {
    id: SessionId,
    pid: u32,
    cwd: PathBuf,
    output: Arc<Mutex<StartupOutput>>,
}

struct Runtime {
    instance: String,
    manager: Arc<SessionManager>,
    sessions: Mutex<HashMap<String, Session>>,
}

impl Runtime {
    fn new() -> Self {
        Self {
            instance: Uuid::new_v4().to_string(),
            manager: Arc::new(SessionManager::default()),
            sessions: Mutex::default(),
        }
    }

    fn execute(&self, request: Request) -> Result<SessionInfo> {
        if request.version != VERSION {
            bail!("unsupported protocol version {}", request.version);
        }
        let mut sessions = lock(&self.sessions)?;
        let active = self.manager.ids()?;
        sessions.retain(|_, session| active.contains(&session.id));
        match request.action {
            Action::Ensure { key, cwd } => {
                let cwd = cwd
                    .canonicalize()
                    .context("resolve shell working directory")?;
                if !cwd.is_dir() || key.is_empty() {
                    bail!("a session key and working directory are required");
                }
                if let Some(existing) = sessions.get(&key) {
                    if existing.cwd != cwd {
                        bail!("session key already belongs to a different working directory");
                    }
                    return existing.info();
                }
                let session = self.start_shell(cwd)?;
                let info = session.info()?;
                sessions.insert(key, session);
                Ok(info)
            }
            Action::Inspect { key } => get(&sessions, &key)?.info(),
            Action::Write { key, text } => {
                let session = get(&sessions, &key)?;
                self.manager.write(session.id, text.as_bytes())?;
                session.info()
            }
            Action::Stop { key } => {
                let session = get(&sessions, &key)?;
                self.manager.kill(session.id)?;
                session.info()
            }
        }
    }

    fn start_shell(&self, cwd: PathBuf) -> Result<Session> {
        let spec = SpawnSpec {
            program: env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned()),
            args: vec!["/d".to_owned()],
            cwd: cwd.clone(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        };
        let output = Arc::new(Mutex::new(StartupOutput::default()));
        let received = Arc::clone(&output);
        let manager = Arc::downgrade(&self.manager);
        let id = self.manager.spawn(
            &spec,
            Arc::new(move |event| {
                if let PtyEvent::Output(bytes) = event {
                    let reply = lock(&received)
                        .ok()
                        .and_then(|mut state| state.record(&bytes));
                    if let (Some(id), Some(manager)) = (reply, manager.upgrade()) {
                        let _ = manager.write(id, b"\x1b[1;1R");
                    }
                }
            }),
        )?;
        let reply = {
            let mut state = lock(&output)?;
            state.id = Some(id);
            state.reply_if_needed()
        };
        if reply.is_some() {
            self.manager.write(id, b"\x1b[1;1R")?;
        }
        let pid = self.manager.process_id(id)?.context("read shell PID")?;
        Ok(Session {
            id,
            pid,
            cwd,
            output,
        })
    }
}

impl Session {
    fn info(&self) -> Result<SessionInfo> {
        Ok(SessionInfo {
            session_id: self.id,
            pid: self.pid,
            output_bytes: lock(&self.output)?.bytes,
        })
    }
}

#[derive(Default)]
struct StartupOutput {
    id: Option<SessionId>,
    bytes: u64,
    matched: usize,
    answered: bool,
}

impl StartupOutput {
    fn record(&mut self, bytes: &[u8]) -> Option<SessionId> {
        self.bytes = self.bytes.saturating_add(bytes.len() as u64);
        if !self.answered && self.matched < CURSOR_QUERY.len() {
            for &byte in bytes {
                if byte == CURSOR_QUERY[self.matched] {
                    self.matched += 1;
                    if self.matched == CURSOR_QUERY.len() {
                        break;
                    }
                } else {
                    self.matched = usize::from(byte == CURSOR_QUERY[0]);
                }
            }
        }
        self.reply_if_needed()
    }

    fn reply_if_needed(&mut self) -> Option<SessionId> {
        if self.answered || self.matched != CURSOR_QUERY.len() {
            return None;
        }
        let id = self.id?;
        self.answered = true;
        Some(id)
    }
}

fn get<'a>(sessions: &'a HashMap<String, Session>, key: &str) -> Result<&'a Session> {
    sessions
        .get(key)
        .with_context(|| format!("no active terminal for {key}"))
}

fn lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>> {
    value
        .lock()
        .map_err(|_| anyhow!("runtime state lock poisoned"))
}

#[tokio::main]
pub(super) async fn run() -> Result<()> {
    let args = Args::parse();
    let name = args.pipe.to_ns_name::<GenericNamespaced>()?;
    match args.command {
        Command::Serve => {
            let descriptor =
                SecurityDescriptor::deserialize(&U16CString::from_str("D:P(A;;GA;;;OW)")?)?;
            let listener = ListenerOptions::new()
                .name(name)
                .security_descriptor(descriptor)
                .create_tokio()?;
            let runtime = Arc::new(Runtime::new());
            loop {
                let stream = listener.accept().await?;
                let runtime = Arc::clone(&runtime);
                tokio::spawn(async move {
                    if let Err(error) =
                        tokio::time::timeout(IO_TIMEOUT, respond(stream, runtime)).await
                    {
                        eprintln!("runtime request timed out: {error}");
                    }
                });
            }
        }
        Command::Send { action } => {
            let stream = Stream::connect(name)
                .await
                .context("connect to terminal runtime")?;
            let response = tokio::time::timeout(IO_TIMEOUT, request(stream, action)).await??;
            println!("{response}");
            if let Some(error) = response.get("error") {
                bail!("{error}");
            }
            Ok(())
        }
    }
}

async fn respond(mut stream: Stream, runtime: Arc<Runtime>) -> Result<()> {
    let message = read_message(&mut stream).await?;
    let result = match serde_json::from_slice::<Request>(&message) {
        Ok(request) => {
            tokio::task::spawn_blocking({
                let runtime = Arc::clone(&runtime);
                move || runtime.execute(request)
            })
            .await?
        }
        Err(error) => Err(error.into()),
    };
    let result = match result {
        Ok(session) => ResponseResult::Success { session },
        Err(error) => ResponseResult::Error {
            error: format!("{error:#}"),
        },
    };
    write_message(
        &mut stream,
        &Response {
            version: VERSION,
            instance: &runtime.instance,
            result,
        },
    )
    .await
}

async fn request(mut stream: Stream, action: Action) -> Result<serde_json::Value> {
    write_message(
        &mut stream,
        &Request {
            version: VERSION,
            action,
        },
    )
    .await?;
    let response: serde_json::Value = serde_json::from_slice(&read_message(&mut stream).await?)?;
    if response.get("version").and_then(serde_json::Value::as_u64) != Some(u64::from(VERSION)) {
        bail!("incompatible runtime response");
    }
    Ok(response)
}

async fn read_message(stream: &mut Stream) -> Result<Vec<u8>> {
    let mut message = Vec::new();
    BufReader::new(stream)
        .take(MAX_MESSAGE)
        .read_until(b'\n', &mut message)
        .await?;
    if message.last() != Some(&b'\n') {
        bail!("missing or oversized runtime message");
    }
    Ok(message)
}

async fn write_message(stream: &mut Stream, value: &impl Serialize) -> Result<()> {
    let mut message = serde_json::to_vec(value)?;
    message.push(b'\n');
    stream.write_all(&message).await?;
    stream.flush().await?;
    Ok(())
}
