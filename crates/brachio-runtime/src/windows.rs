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
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;
use uuid::Uuid;
use widestring::U16CString;

use crate::output::{OutputHistory, Page};
use crate::screen::{TerminalScreen, COLS, ROWS};

const VERSION: u8 = 1;
const MAX_MESSAGE: u64 = 64 * 1024;
const MAX_RESPONSE: u64 = 256 * 1024;
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
    /// Read a page of output starting at the requested absolute byte offset.
    Read { key: String, after: u64 },
    /// Replay and then follow output on the same connection.
    Follow { key: String, after: u64 },
    /// Capture the current visible frame of a running shell.
    Snapshot { key: String },
    /// Send a settled normal-screen frame, then follow future output.
    FollowFrame { key: String },
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
    Success {
        session: SessionInfo,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<OutputPage>,
    },
    Snapshot {
        session: SessionInfo,
        screen: ScreenSnapshot,
    },
    Error {
        error: String,
    },
}

#[derive(Serialize)]
struct OutputPage {
    start: u64,
    next: u64,
    end: u64,
    truncated: bool,
    bytes: Vec<u8>,
}

impl From<Page> for OutputPage {
    fn from(page: Page) -> Self {
        Self {
            start: page.start,
            next: page.next,
            end: page.end,
            truncated: page.truncated,
            bytes: page.bytes,
        }
    }
}

#[derive(Serialize)]
struct SessionInfo {
    session_id: SessionId,
    pid: u32,
    output_bytes: u64,
    exited: bool,
    exit_code: Option<u32>,
}

#[derive(Serialize)]
struct ScreenSnapshot {
    ansi: String,
    cursor: (u16, u16),
    rows: u16,
    cols: u16,
    alternate_screen: bool,
    resume_offset: Option<u64>,
}

#[derive(Clone)]
struct Session {
    id: SessionId,
    pid: u32,
    cwd: PathBuf,
    output: Arc<Mutex<StartupOutput>>,
    updates: watch::Receiver<()>,
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

    fn execute(&self, request: Request) -> Result<ResponseResult> {
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
                    return existing.describe(None);
                }
                let session = self.start_shell(cwd)?;
                let info = session.describe(None)?;
                sessions.insert(key, session);
                Ok(info)
            }
            Action::Inspect { key } => get(&sessions, &key)?.describe(None),
            Action::Read { key, after } => get(&sessions, &key)?.describe(Some(after)),
            Action::Snapshot { key } => get(&sessions, &key)?.snapshot(),
            Action::Follow { .. } | Action::FollowFrame { .. } => {
                bail!("follow requires a streaming response");
            }
            Action::Write { key, text } => {
                let session = get(&sessions, &key)?;
                self.manager.write(session.id, text.as_bytes())?;
                session.describe(None)
            }
            Action::Stop { key } => {
                let session = get(&sessions, &key)?;
                self.manager.kill(session.id)?;
                session.describe(None)
            }
        }
    }

    fn start_shell(&self, cwd: PathBuf) -> Result<Session> {
        let spec = SpawnSpec {
            program: env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned()),
            args: vec!["/d".to_owned()],
            cwd: cwd.clone(),
            env: Vec::new(),
            rows: ROWS.get(),
            cols: COLS.get(),
        };
        let output = Arc::new(Mutex::new(StartupOutput::default()));
        let received = Arc::clone(&output);
        let (updates, receiver) = watch::channel(());
        let manager = Arc::downgrade(&self.manager);
        let id = self.manager.spawn(
            &spec,
            Arc::new(move |event| match event {
                PtyEvent::Output(bytes) => {
                    let reply = lock(&received)
                        .ok()
                        .and_then(|mut state| state.record(&bytes));
                    updates.send_replace(());
                    if let (Some(id), Some(manager)) = (reply, manager.upgrade()) {
                        let _ = manager.write(id, b"\x1b[1;1R");
                    }
                }
                PtyEvent::Exit(code) => {
                    if let Ok(mut state) = lock(&received) {
                        state.exited = true;
                        state.exit_code = code;
                    }
                    updates.send_replace(());
                }
                PtyEvent::Status(_) => {}
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
            updates: receiver,
        })
    }
}

impl Session {
    fn describe(&self, after: Option<u64>) -> Result<ResponseResult> {
        let state = lock(&self.output)?;
        let output = after
            .map(|offset| {
                state
                    .history
                    .read(offset)
                    .map(OutputPage::from)
                    .context("output offset is past the end")
            })
            .transpose()?;
        Ok(ResponseResult::Success {
            session: self.info(&state),
            output,
        })
    }

    fn snapshot(&self) -> Result<ResponseResult> {
        let state = lock(&self.output)?;
        let frame = state.screen.frame();
        let resume_offset =
            (frame.at_boundary && !frame.alternate_screen).then(|| state.history.end());
        Ok(ResponseResult::Snapshot {
            session: self.info(&state),
            screen: ScreenSnapshot {
                ansi: frame.ansi,
                cursor: frame.cursor,
                rows: ROWS.get(),
                cols: COLS.get(),
                alternate_screen: frame.alternate_screen,
                resume_offset,
            },
        })
    }

    fn info(&self, state: &StartupOutput) -> SessionInfo {
        SessionInfo {
            session_id: self.id,
            pid: self.pid,
            output_bytes: state.history.end(),
            exited: state.exited,
            exit_code: state.exit_code,
        }
    }
}

#[derive(Default)]
struct StartupOutput {
    id: Option<SessionId>,
    history: OutputHistory,
    screen: TerminalScreen,
    matched: usize,
    answered: bool,
    exited: bool,
    exit_code: Option<u32>,
}

impl StartupOutput {
    fn record(&mut self, bytes: &[u8]) -> Option<SessionId> {
        self.history.push(bytes);
        self.screen.process(bytes);
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
        Command::Send { action } => {
            let stream = Stream::connect(name)
                .await
                .context("connect to terminal runtime")?;
            request(stream, action).await
        }
    }
}

async fn respond(mut stream: Stream, runtime: Arc<Runtime>) -> Result<()> {
    let message = tokio::time::timeout(
        IO_TIMEOUT,
        read_message(&mut BufReader::new(&mut stream), MAX_MESSAGE),
    )
    .await??
    .context("connection closed before request")?;
    let result = match serde_json::from_slice::<Request>(&message) {
        Ok(Request {
            version: VERSION,
            action: Action::Follow { key, after },
        }) => {
            let session =
                lock(&runtime.sessions).and_then(|sessions| get(&sessions, &key).cloned());
            match session {
                Ok(session) => return follow(&mut stream, &runtime.instance, session, after).await,
                Err(error) => Err(error),
            }
        }
        Ok(Request {
            version: VERSION,
            action: Action::FollowFrame { key },
        }) => {
            let session =
                lock(&runtime.sessions).and_then(|sessions| get(&sessions, &key).cloned());
            match session {
                Ok(session) => {
                    let snapshot = session.snapshot()?;
                    if let ResponseResult::Snapshot { screen, .. } = &snapshot {
                        if let Some(offset) = screen.resume_offset {
                            send_response(&mut stream, &runtime.instance, Ok(snapshot)).await?;
                            return follow(&mut stream, &runtime.instance, session, offset).await;
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
        Err(error) => Err(error.into()),
    };
    send_response(&mut stream, &runtime.instance, result).await
}

async fn send_response(
    stream: &mut Stream,
    instance: &str,
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
                version: VERSION,
                instance,
                result,
            },
            MAX_RESPONSE,
        ),
    )
    .await?
}

async fn follow(
    stream: &mut Stream,
    instance: &str,
    mut session: Session,
    mut offset: u64,
) -> Result<()> {
    let mut first = true;
    loop {
        // Mark updates before reading the journal. A write that arrives after
        // the snapshot remains observable when we begin waiting.
        session.updates.borrow_and_update();
        let result = session.describe(Some(offset));
        let (next, end, exited) = match &result {
            Ok(ResponseResult::Success {
                session,
                output: Some(page),
            }) => (page.next, page.end, session.exited),
            _ => return send_response(stream, instance, result).await,
        };
        if first || next != offset || exited {
            send_response(stream, instance, result).await?;
        }
        first = false;
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

async fn request(mut stream: Stream, action: Action) -> Result<()> {
    let following = matches!(action, Action::Follow { .. } | Action::FollowFrame { .. });
    tokio::time::timeout(
        IO_TIMEOUT,
        write_message(
            &mut stream,
            &Request {
                version: VERSION,
                action,
            },
            MAX_MESSAGE,
        ),
    )
    .await??;
    let mut reader = BufReader::new(stream);
    let first = tokio::time::timeout(IO_TIMEOUT, read_message(&mut reader, MAX_RESPONSE))
        .await??
        .context("runtime closed before responding")?;
    let completed = print_response(&first)?;
    if following {
        if completed {
            return Ok(());
        }
        while let Some(message) = read_message(&mut reader, MAX_RESPONSE).await? {
            if print_response(&message)? {
                return Ok(());
            }
        }
        bail!("runtime disconnected before the terminal exited");
    }
    Ok(())
}

fn print_response(message: &[u8]) -> Result<bool> {
    use std::io::Write;

    let response: serde_json::Value = serde_json::from_slice(message)?;
    if response.get("version").and_then(serde_json::Value::as_u64) != Some(u64::from(VERSION)) {
        bail!("incompatible runtime response");
    }
    let mut output = std::io::stdout().lock();
    output.write_all(message)?;
    output.flush()?;
    if let Some(error) = response.get("error") {
        bail!("{error}");
    }
    let output = &response["output"];
    let completed = response["session"]["exited"] == true
        && output["next"]
            .as_u64()
            .is_some_and(|next| Some(next) == output["end"].as_u64());
    Ok(completed)
}

async fn read_message(
    reader: &mut (impl AsyncBufRead + Unpin),
    limit: u64,
) -> Result<Option<Vec<u8>>> {
    let mut message = Vec::new();
    reader.take(limit).read_until(b'\n', &mut message).await?;
    if message.is_empty() {
        return Ok(None);
    }
    if message.last() != Some(&b'\n') {
        bail!("missing or oversized runtime message");
    }
    Ok(Some(message))
}

async fn write_message(stream: &mut Stream, value: &impl Serialize, limit: u64) -> Result<()> {
    let mut message = serde_json::to_vec(value)?;
    message.push(b'\n');
    if message.len() as u64 > limit {
        bail!("runtime message exceeds the frame limit");
    }
    stream.write_all(&message).await?;
    stream.flush().await?;
    Ok(())
}
