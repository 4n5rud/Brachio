//! Local control of an independent PTY runtime: the server, its sessions and the client.

use std::collections::{HashMap, HashSet};
use std::env;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use brachio_pty::{PtyEvent, SessionId, SessionManager, SpawnSpec};
use brachio_runtime::endpoint::{self, Liveness};
use brachio_runtime::protocol;
use brachio_runtime::transport;
use brachio_runtime::wire::{
    accepted, read_message, write_message, Action, LostInfo, OutputPage, Request, Response,
    ResponseResult, RuntimeInfo, ScreenSnapshot, SessionInfo, IO_TIMEOUT, MAX_MESSAGE,
    MAX_RESPONSE,
};
use clap::{Parser, Subcommand};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::Stream;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::sync::watch;
use uuid::Uuid;

use crate::journal::{self, Journal, SessionRecord};
use crate::output::{OutputHistory, Page};
use crate::screen::{TerminalScreen, COLS, ROWS};
use std::num::NonZeroU16;

const CURSOR_QUERY: &[u8] = b"\x1b[6n";

#[derive(Parser)]
struct Args {
    /// Local pipe name for this runtime instance.
    #[arg(long)]
    pipe: String,
    /// Where the last screen of each shell is kept. Defaults to the user's state directory.
    #[arg(long)]
    state_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Own PTYs until this process exits.
    Serve,
    /// Send one request to a running instance.
    Send {
        /// Protocol version to speak, for testing older clients.
        #[arg(long, default_value_t = protocol::CURRENT)]
        protocol: u8,
        #[command(subcommand)]
        action: Action,
    },
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
    draining: AtomicBool,
    journal: Option<Journal>,
    /// Shells that are gone but whose last screen can still be read.
    lost: Mutex<HashMap<String, SessionRecord>>,
    /// Keys the user stopped on purpose; their records are deleted, not kept.
    stopped: Mutex<HashSet<String>>,
    /// What the journal already holds for each key: output length and exit state.
    saved: Mutex<HashMap<String, (u64, bool)>>,
}

impl Runtime {
    fn new(journal: Option<Journal>) -> Self {
        let mut lost = HashMap::new();
        if let Some(journal) = &journal {
            match journal.load() {
                Ok(loaded) => {
                    for path in &loaded.unreadable {
                        eprintln!("set aside unreadable journal file {}", path.display());
                    }
                    lost.extend(
                        loaded
                            .records
                            .into_iter()
                            .map(|record| (record.key.clone(), record)),
                    );
                }
                Err(error) => eprintln!("cannot read the journal: {error}"),
            }
        }
        Self {
            instance: Uuid::new_v4().to_string(),
            manager: Arc::new(SessionManager::default()),
            sessions: Mutex::default(),
            draining: AtomicBool::new(false),
            journal,
            lost: Mutex::new(lost),
            stopped: Mutex::default(),
            saved: Mutex::default(),
        }
    }

    /// Write the screen of every shell that changed since the last write.
    fn persist(&self) {
        let Some(journal) = &self.journal else { return };
        let records: Vec<SessionRecord> = match lock(&self.sessions) {
            Ok(sessions) => sessions
                .iter()
                .filter_map(|(key, session)| session.record(key, &self.instance).ok())
                .collect(),
            Err(_) => return,
        };
        let Ok(mut saved) = lock(&self.saved) else {
            return;
        };
        for record in records {
            let mark = (record.output_bytes, record.exited);
            if saved.get(&record.key) != Some(&mark) && journal.save(&record).is_ok() {
                saved.insert(record.key, mark);
            }
        }
    }

    /// A shell that ended by itself keeps its last screen, unless the user stopped it.
    fn keep_ended(&self, key: &str, session: &Session) -> Result<()> {
        if lock(&self.stopped)?.remove(key) {
            return Ok(());
        }
        let mut record = session.record(key, &self.instance)?;
        record.exited = true;
        if let Some(journal) = &self.journal {
            journal.save(&record)?;
        }
        lock(&self.saved)?.remove(key);
        lock(&self.lost)?.insert(key.to_owned(), record);
        Ok(())
    }

    fn forget(&self, key: &str) -> Result<()> {
        lock(&self.lost)?.remove(key);
        lock(&self.saved)?.remove(key);
        if let Some(journal) = &self.journal {
            journal.remove(key)?;
        }
        Ok(())
    }

    /// A draining runtime with no shells left has nothing more to serve.
    fn finished(&self) -> bool {
        self.draining.load(Ordering::SeqCst) && self.manager.ids().is_ok_and(|ids| ids.is_empty())
    }

    fn hello(&self) -> ResponseResult {
        ResponseResult::Hello {
            runtime: RuntimeInfo {
                pid: std::process::id(),
                oldest_version: protocol::OLDEST,
                current_version: protocol::CURRENT,
                draining: self.draining.load(Ordering::SeqCst),
            },
        }
    }

    fn execute(&self, request: Request) -> Result<ResponseResult> {
        let mut sessions = lock(&self.sessions)?;
        let active = self.manager.ids()?;
        let ended: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| !active.contains(&session.id))
            .map(|(key, _)| key.clone())
            .collect();
        for key in ended {
            if let Some(session) = sessions.remove(&key) {
                if let Err(error) = self.keep_ended(&key, &session) {
                    eprintln!("cannot keep the last screen of {key}: {error:#}");
                }
            }
        }
        match request.action {
            Action::Hello => Ok(self.hello()),
            Action::Lost => {
                let lost = lock(&self.lost)?;
                let mut listed: Vec<LostInfo> = lost.values().map(lost_info).collect();
                listed.sort_by(|a, b| a.key.cmp(&b.key));
                Ok(ResponseResult::Lost { lost: listed })
            }
            Action::Recover { key } => {
                let lost = lock(&self.lost)?;
                let record = lost
                    .get(&key)
                    .with_context(|| format!("no saved screen for {key}"))?;
                Ok(ResponseResult::Recovered {
                    lost: lost_info(record),
                    screen: ScreenSnapshot {
                        ansi: record.ansi.clone(),
                        cursor: record.cursor,
                        rows: record.rows,
                        cols: record.cols,
                        alternate_screen: record.alternate_screen,
                        resume_offset: None,
                    },
                })
            }
            Action::Forget { key } => {
                self.forget(&key)?;
                Ok(self.hello())
            }
            Action::Drain => {
                self.draining.store(true, Ordering::SeqCst);
                Ok(self.hello())
            }
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
                if self.draining.load(Ordering::SeqCst) {
                    bail!("runtime is draining and takes no new terminals");
                }
                let session = self.start_shell(cwd)?;
                self.forget(&key)?;
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
            Action::Resize { key, rows, cols } => {
                let session = get(&sessions, &key)?;
                let rows = NonZeroU16::new(rows).context("rows must be at least 1")?;
                let cols = NonZeroU16::new(cols).context("columns must be at least 1")?;
                self.manager.resize(session.id, rows.get(), cols.get())?;
                lock(&session.output)?.screen.resize(rows, cols);
                session.describe(None)
            }
            Action::Write { key, text } => {
                let session = get(&sessions, &key)?;
                self.manager.write(session.id, text.as_bytes())?;
                session.describe(None)
            }
            Action::Stop { key } => {
                let session = get(&sessions, &key)?;
                lock(&self.stopped)?.insert(key.clone());
                self.forget(&key)?;
                self.manager.kill(session.id)?;
                session.describe(None)
            }
        }
    }

    fn start_shell(&self, cwd: PathBuf) -> Result<Session> {
        let shell = shell_command();
        let spec = SpawnSpec {
            program: shell.0,
            args: shell.1,
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
                    .map(output_page)
                    .context("output offset is past the end")
            })
            .transpose()?;
        Ok(ResponseResult::Success {
            session: self.info(&state),
            output,
        })
    }

    fn record(&self, key: &str, instance: &str) -> Result<SessionRecord> {
        let state = lock(&self.output)?;
        let frame = state.screen.frame();
        let saved_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        Ok(SessionRecord {
            key: key.to_owned(),
            cwd: self.cwd.clone(),
            instance: instance.to_owned(),
            pid: self.pid,
            rows: frame.rows,
            cols: frame.cols,
            ansi: frame.ansi,
            cursor: frame.cursor,
            alternate_screen: frame.alternate_screen,
            output_bytes: state.history.end(),
            exited: state.exited,
            exit_code: state.exit_code,
            saved_at_ms,
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
                rows: frame.rows,
                cols: frame.cols,
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

#[cfg(windows)]
fn shell_command() -> (String, Vec<String>) {
    let program = env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned());
    (program, vec!["/d".to_owned()])
}

#[cfg(unix)]
fn shell_command() -> (String, Vec<String>) {
    let program = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    (program, Vec::new())
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
    match args.command {
        Command::Serve => serve(&args.pipe, args.state_dir).await,
        Command::Send { protocol, action } => {
            let connected = transport::connect(&args.pipe).await;
            let reason = match endpoint::classify(&connected) {
                Liveness::Dead => "terminal runtime is not running",
                _ => "cannot tell whether the terminal runtime is running",
            };
            request(connected.context(reason)?, protocol, action).await
        }
    }
}

async fn serve(pipe: &str, state_dir: Option<PathBuf>) -> Result<()> {
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
async fn own(pipe: &str) -> Result<interprocess::local_socket::tokio::Listener, (i32, io::Error)> {
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

async fn respond(mut stream: Stream, runtime: Arc<Runtime>) -> Result<()> {
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

async fn send_response(
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

async fn follow(
    stream: &mut Stream,
    instance: &str,
    version: u8,
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
            _ => return send_response(stream, instance, version, result).await,
        };
        if first || next != offset || exited {
            send_response(stream, instance, version, result).await?;
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

async fn request(mut stream: Stream, protocol: u8, action: Action) -> Result<()> {
    let following = matches!(action, Action::Follow { .. } | Action::FollowFrame { .. });
    tokio::time::timeout(
        IO_TIMEOUT,
        write_message(
            &mut stream,
            &Request {
                version: protocol,
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
    let completed = print_response(&first, protocol)?;
    if following {
        if completed {
            return Ok(());
        }
        while let Some(message) = read_message(&mut reader, MAX_RESPONSE).await? {
            if print_response(&message, protocol)? {
                return Ok(());
            }
        }
        bail!("runtime disconnected before the terminal exited");
    }
    Ok(())
}

fn print_response(message: &[u8], protocol: u8) -> Result<bool> {
    use std::io::Write;

    let response: serde_json::Value = serde_json::from_slice(message)?;
    let mut output = std::io::stdout().lock();
    output.write_all(message)?;
    output.flush()?;
    if let Some(error) = response.get("error") {
        bail!("{error}");
    }
    if response.get("version").and_then(serde_json::Value::as_u64) != Some(u64::from(protocol)) {
        bail!("incompatible runtime response");
    }
    let output = &response["output"];
    let completed = response["session"]["exited"] == true
        && output["next"]
            .as_u64()
            .is_some_and(|next| Some(next) == output["end"].as_u64());
    Ok(completed)
}

fn lost_info(record: &SessionRecord) -> LostInfo {
    LostInfo {
        key: record.key.clone(),
        cwd: record.cwd.clone(),
        former_instance: record.instance.clone(),
        former_pid: record.pid,
        exited: record.exited,
        exit_code: record.exit_code,
        output_bytes: record.output_bytes,
        saved_at_ms: record.saved_at_ms,
    }
}

fn output_page(page: Page) -> OutputPage {
    OutputPage {
        start: page.start,
        next: page.next,
        end: page.end,
        truncated: page.truncated,
        bytes: page.bytes,
    }
}
