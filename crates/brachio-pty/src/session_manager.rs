//! Owns every live PTY and streams its output to the UI.

use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use brachio_core::status::{OscScanner, ProgramStatus, ProgramStatusMessage, QUERY_REPLY};
use serde::{Deserialize, Serialize};

use crate::exit_watch::ExitWatch;
use crate::output_batcher::{BatchConfig, OutputBatcher};
use crate::pty_session::{PtySession, SpawnSpec};

/// Identifies one PTY session for its lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(u64);

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// What a running session reports to its sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyEvent {
    /// A batch of raw terminal output.
    Output(Vec<u8>),
    /// The child exited. `None` when the exit code could not be read.
    Exit(Option<u32>),
    /// The program reported its state through OSC 7501.
    Status(ProgramStatus),
}

/// Receives events for one session. Called from background threads.
pub type EventSink = Arc<dyn Fn(PtyEvent) + Send + Sync>;

/// A session shared by the manager's table and its flusher thread.
pub(crate) type SharedSession = Arc<Mutex<PtySession>>;

/// Owns every live PTY.
///
/// Each session gets two threads: a reader, because PTY reads block, and a
/// flusher, which turns raw chunks into batched frames and reports the exit.
#[derive(Default)]
pub struct SessionManager {
    last_id: AtomicU64,
    sessions: Mutex<HashMap<SessionId, SharedSession>>,
    batch_config: BatchConfig,
}

impl SessionManager {
    /// A manager that batches output with `batch_config`.
    #[must_use]
    pub fn new(batch_config: BatchConfig) -> Self {
        Self {
            batch_config,
            ..Self::default()
        }
    }

    /// Starts `spec` and streams its output to `sink` until it exits.
    ///
    /// # Errors
    /// Fails when the PTY or its threads cannot be started.
    pub fn spawn(self: &Arc<Self>, spec: &SpawnSpec, sink: EventSink) -> Result<SessionId> {
        let session = PtySession::spawn(spec)?;
        let reader = session.take_reader()?;
        let id = SessionId(self.last_id.fetch_add(1, Ordering::Relaxed) + 1);
        let shared = Arc::new(Mutex::new(session));
        self.table()?.insert(id, Arc::clone(&shared));
        if let Err(error) = self.start_threads(id, &shared, reader, sink) {
            // Without its threads nobody would ever notice the child ending, so it must not stay.
            self.forget(id);
            if let Ok(mut session) = lock_session(&shared) {
                let _ = session.kill();
            }
            return Err(error);
        }
        Ok(id)
    }

    fn start_threads(
        self: &Arc<Self>,
        id: SessionId,
        shared: &SharedSession,
        reader: Box<dyn Read + Send>,
        sink: EventSink,
    ) -> Result<()> {
        let (chunk_sender, chunk_receiver) = mpsc::channel();
        let status = StatusWatch {
            session: Arc::downgrade(shared),
            sink: Arc::clone(&sink),
            scanner: OscScanner::new(),
        };
        thread::Builder::new()
            .name(format!("pty-read-{id}"))
            .spawn(move || read_until_closed(reader, &chunk_sender, status))
            .context("start pty reader")?;

        let manager = Arc::clone(self);
        let batch_config = self.batch_config;
        let shared = Arc::clone(shared);
        let forget = Box::new(move || manager.forget(id));
        thread::Builder::new()
            .name(format!("pty-flush-{id}"))
            .spawn(move || {
                let mut exit_watch = ExitWatch::new(shared, forget);
                flush_until_done(&chunk_receiver, batch_config, &sink, &mut exit_watch);
                exit_watch.release();
                sink(PtyEvent::Exit(exit_watch.finish()));
            })
            .context("start pty flusher")?;
        Ok(())
    }

    /// Sends keyboard input to a session.
    ///
    /// # Errors
    /// Fails when the session has ended or its PTY is closed.
    pub fn write(&self, id: SessionId, bytes: &[u8]) -> Result<()> {
        lock_session(&self.get(id)?)?.write_input(bytes)
    }

    /// Changes a session's terminal size.
    ///
    /// # Errors
    /// Fails when the session has ended or its PTY is closed.
    pub fn resize(&self, id: SessionId, rows: u16, cols: u16) -> Result<()> {
        lock_session(&self.get(id)?)?.resize(rows, cols)
    }

    /// Kills the child. The exit event still arrives through the sink.
    ///
    /// # Errors
    /// Fails when the session has ended or the child cannot be signalled.
    pub fn kill(&self, id: SessionId) -> Result<()> {
        lock_session(&self.get(id)?)?.kill()
    }

    /// Ids of sessions whose child has not exited yet, oldest first.
    ///
    /// # Errors
    /// Fails when the session table is poisoned.
    pub fn ids(&self) -> Result<Vec<SessionId>> {
        let mut ids: Vec<SessionId> = self.table()?.keys().copied().collect();
        ids.sort_unstable();
        Ok(ids)
    }

    /// Operating-system identifier for an active child, when available.
    ///
    /// # Errors
    /// Fails when the session has ended or cannot be locked.
    pub fn process_id(&self, id: SessionId) -> Result<Option<u32>> {
        Ok(lock_session(&self.get(id)?)?.process_id())
    }

    fn get(&self, id: SessionId) -> Result<SharedSession> {
        let table = self.table()?;
        let session = table
            .get(&id)
            .ok_or_else(|| anyhow!("no pty session {id}"))?;
        Ok(Arc::clone(session))
    }

    fn forget(&self, id: SessionId) {
        if let Ok(mut table) = self.table() {
            table.remove(&id);
        }
    }

    fn table(&self) -> Result<MutexGuard<'_, HashMap<SessionId, SharedSession>>> {
        self.sessions
            .lock()
            .map_err(|_| anyhow!("session table poisoned"))
    }
}

/// Locks a session without reaching into a poisoned mutex.
pub(crate) fn lock_session(session: &SharedSession) -> Result<MutexGuard<'_, PtySession>> {
    session.lock().map_err(|_| anyhow!("pty session poisoned"))
}

/// Reads OSC 7501 status out of the raw output, before batching.
///
/// Runs on the reader thread so a query is answered ahead of the bytes that
/// follow it. Programs send a device attributes request right after the
/// query and expect this answer to arrive first.
struct StatusWatch {
    // The reader must not keep the ConPTY master alive after child exit.
    session: Weak<Mutex<PtySession>>,
    sink: EventSink,
    scanner: OscScanner,
}

impl StatusWatch {
    fn inspect(&mut self, bytes: &[u8]) {
        let Self {
            session,
            sink,
            scanner,
        } = self;
        scanner.feed(bytes, |body| match ProgramStatusMessage::parse(body) {
            Some(ProgramStatusMessage::Query) => {
                if let Some(session) = session.upgrade() {
                    if let Ok(session) = lock_session(&session) {
                        // A failed write means the PTY is closing; the exit event follows.
                        let _ = session.write_input(QUERY_REPLY);
                    }
                }
            }
            Some(ProgramStatusMessage::Report(status)) => sink(PtyEvent::Status(status)),
            None => {}
        });
    }
}

fn read_until_closed(
    mut reader: Box<dyn Read + Send>,
    chunks: &Sender<Vec<u8>>,
    mut status: StatusWatch,
) {
    let mut buffer = vec![0u8; 16 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        status.inspect(&buffer[..read]);
        if chunks.send(buffer[..read].to_vec()).is_err() {
            return;
        }
    }
}

/// Batches chunks until the reader hangs up or the exit watch gives up on
/// it, then flushes what is left.
fn flush_until_done(
    chunks: &Receiver<Vec<u8>>,
    config: BatchConfig,
    sink: &EventSink,
    exit_watch: &mut ExitWatch,
) {
    let start = Instant::now();
    let now_ms = || u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut batcher = OutputBatcher::new(config);
    let wait = Duration::from_millis(config.max_delay_ms.max(1));
    loop {
        let frame = match chunks.recv_timeout(wait) {
            Ok(chunk) => batcher
                .push(&chunk, now_ms())
                .or_else(|| batcher.tick(now_ms())),
            Err(RecvTimeoutError::Timeout) if exit_watch.should_stop() => break,
            Err(RecvTimeoutError::Timeout) => batcher.tick(now_ms()),
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if let Some(frame) = frame {
            sink(PtyEvent::Output(frame));
        }
    }
    drain_remaining(chunks, &mut batcher, &now_ms, sink);
}

/// Sends everything still queued, including the frames `push` completes on the way.
fn drain_remaining(
    chunks: &Receiver<Vec<u8>>,
    batcher: &mut OutputBatcher,
    now_ms: &dyn Fn() -> u64,
    sink: &EventSink,
) {
    while let Ok(chunk) = chunks.try_recv() {
        if let Some(frame) = batcher.push(&chunk, now_ms()) {
            sink(PtyEvent::Output(frame));
        }
    }
    if let Some(frame) = batcher.tick(u64::MAX) {
        sink(PtyEvent::Output(frame));
    }
}

#[cfg(test)]
mod drain_tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    use super::drain_remaining;
    use crate::{BatchConfig, OutputBatcher, PtyEvent};

    #[test]
    fn every_queued_byte_is_sent_in_order_even_when_frames_complete_while_draining() {
        let (sender, receiver) = mpsc::channel();
        let mut expected = Vec::new();
        for index in 0..300u32 {
            let chunk: Vec<u8> = (0..1_000)
                .map(|offset| ((index + offset) % 251) as u8)
                .collect();
            expected.extend_from_slice(&chunk);
            sender.send(chunk).expect("queue chunk");
        }
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink_received = Arc::clone(&received);
        let sink: crate::EventSink = Arc::new(move |event| {
            if let PtyEvent::Output(bytes) = event {
                sink_received.lock().expect("lock").extend(bytes);
            }
        });
        let mut batcher = OutputBatcher::new(BatchConfig::default());
        drain_remaining(&receiver, &mut batcher, &|| 0, &sink);
        assert_eq!(*received.lock().expect("lock"), expected);
    }
}

#[cfg(all(test, unix))]
mod stalled_child_tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::{SessionManager, SpawnSpec};

    #[test]
    fn a_child_that_never_reads_cannot_stall_writes_or_other_calls_on_its_session() {
        let manager = Arc::new(SessionManager::default());
        let spec = SpawnSpec {
            program: "/bin/sleep".to_owned(),
            args: vec!["30".to_owned()],
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        };
        let id = manager
            .spawn(&spec, Arc::new(|_| {}))
            .expect("start a child that ignores its input");

        // A megabyte is far more than a PTY accepts from a child that is not reading.
        let started = Instant::now();
        manager
            .write(id, &vec![b'x'; 1 << 20])
            .expect("queue input");
        manager
            .resize(id, 30, 100)
            .expect("resize while input is pending");
        assert!(manager.process_id(id).expect("process id").is_some());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "calls waited on a child that does not read: {:?}",
            started.elapsed()
        );
        manager.kill(id).expect("kill");
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::{PtyEvent, SessionManager, SpawnSpec};

    #[test]
    fn conpty_reports_output_and_exit_after_cursor_response() {
        let manager = Arc::new(SessionManager::default());
        let (sender, receiver) = mpsc::channel();
        let spec = SpawnSpec {
            program: "cmd.exe".to_owned(),
            args: ["/d", "/c", "echo brachio-win-pty"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        };
        let id = manager
            .spawn(
                &spec,
                Arc::new(move |event| {
                    let _ = sender.send(event);
                }),
            )
            .expect("start ConPTY child");
        let end = Instant::now() + Duration::from_secs(10);
        let mut output = Vec::new();
        let mut cursor_report_sent = false;
        while let Some(remaining) = end.checked_duration_since(Instant::now()) {
            match receiver.recv_timeout(remaining) {
                Ok(PtyEvent::Output(bytes)) => {
                    output.extend(bytes);
                    if !cursor_report_sent && output.windows(4).any(|bytes| bytes == b"\x1b[6n") {
                        manager
                            .write(id, b"\x1b[1;1R")
                            .expect("answer cursor request");
                        cursor_report_sent = true;
                    }
                }
                Ok(PtyEvent::Exit(code)) => {
                    assert_eq!(code, Some(0));
                    assert!(String::from_utf8_lossy(&output).contains("brachio-win-pty"));
                    assert!(manager.ids().expect("list active sessions").is_empty());
                    return;
                }
                Ok(PtyEvent::Status(_)) => {}
                Err(_) => break,
            }
        }
        let _ = manager.kill(id);
        panic!("ConPTY child did not report exit");
    }

    #[test]
    fn a_shell_starts_in_a_folder_given_in_the_verbatim_form_canonicalize_returns() {
        let folder = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temp dir");
        assert!(folder.to_string_lossy().starts_with(r"\\?\"));
        let plain = folder
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase();

        let manager = Arc::new(SessionManager::default());
        let (sender, receiver) = mpsc::channel();
        let spec = SpawnSpec {
            program: "cmd.exe".to_owned(),
            args: ["/d", "/c", "cd"].into_iter().map(str::to_owned).collect(),
            cwd: folder,
            env: Vec::new(),
            rows: 24,
            cols: 200,
        };
        let id = manager
            .spawn(
                &spec,
                Arc::new(move |event| {
                    let _ = sender.send(event);
                }),
            )
            .expect("start ConPTY child");
        let end = Instant::now() + Duration::from_secs(10);
        let mut output = Vec::new();
        let mut cursor_report_sent = false;
        while let Some(remaining) = end.checked_duration_since(Instant::now()) {
            match receiver.recv_timeout(remaining) {
                Ok(PtyEvent::Output(bytes)) => {
                    output.extend(bytes);
                    if !cursor_report_sent && output.windows(4).any(|bytes| bytes == b"\x1b[6n") {
                        manager
                            .write(id, b"\x1b[1;1R")
                            .expect("answer cursor request");
                        cursor_report_sent = true;
                    }
                }
                Ok(PtyEvent::Exit(_)) => {
                    let text = String::from_utf8_lossy(&output).to_lowercase();
                    assert!(
                        !text.contains(r"\\?\"),
                        "cmd complained about the verbatim path: {text:?}"
                    );
                    assert!(
                        text.contains(&plain),
                        "cmd did not start in {plain}: {text:?}"
                    );
                    return;
                }
                Ok(PtyEvent::Status(_)) => {}
                Err(_) => break,
            }
        }
        let _ = manager.kill(id);
        panic!("ConPTY child did not report exit");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(5);

    fn spec(program: &str, args: &[&str]) -> SpawnSpec {
        SpawnSpec {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        }
    }

    fn spawn_collecting(
        manager: &Arc<SessionManager>,
        spec: &SpawnSpec,
    ) -> (SessionId, Receiver<PtyEvent>) {
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        let sink: EventSink = Arc::new(move |event| {
            if let Ok(sender) = sender.lock() {
                let _ = sender.send(event);
            }
        });
        let id = manager.spawn(spec, sink).expect("spawn session");
        (id, receiver)
    }

    /// Collects output until the exit event.
    fn wait_for_exit(events: &Receiver<PtyEvent>) -> (String, Option<u32>) {
        let mut output = Vec::new();
        loop {
            match events.recv_timeout(TIMEOUT).expect("session ends in time") {
                PtyEvent::Output(bytes) => output.extend(bytes),
                PtyEvent::Exit(code) => return (String::from_utf8_lossy(&output).into(), code),
                PtyEvent::Status(_) => {}
            }
        }
    }

    #[test]
    fn streams_output_then_exit_code() {
        let manager = Arc::new(SessionManager::default());
        let script = spec("/bin/sh", &["-c", "echo brachio-out; exit 3"]);
        let (_, events) = spawn_collecting(&manager, &script);

        let (output, code) = wait_for_exit(&events);

        assert!(output.contains("brachio-out"), "output missing: {output:?}");
        assert_eq!(code, Some(3));
        assert!(
            manager.ids().expect("list ids").is_empty(),
            "exited sessions are forgotten"
        );
    }

    #[test]
    fn input_reaches_the_child_and_kill_ends_it() {
        let manager = Arc::new(SessionManager::default());
        let (id, events) = spawn_collecting(&manager, &spec("/bin/cat", &[]));

        manager.write(id, b"ping-brachio\n").expect("write input");
        let mut echoed = Vec::new();
        while !String::from_utf8_lossy(&echoed).contains("ping-brachio") {
            if let PtyEvent::Output(bytes) = events.recv_timeout(TIMEOUT).expect("echo in time") {
                echoed.extend(bytes);
            }
        }
        manager.resize(id, 40, 120).expect("resize");
        manager.kill(id).expect("kill");
        wait_for_exit(&events);

        assert!(
            manager.write(id, b"x").is_err(),
            "a killed session takes no input"
        );
    }

    #[test]
    fn reports_program_status() {
        let manager = Arc::new(SessionManager::default());
        let script = spec(
            "/bin/sh",
            &["-c", r"printf '\033]7501;state=working\033\\'"],
        );
        let (_, events) = spawn_collecting(&manager, &script);

        let status = loop {
            match events.recv_timeout(TIMEOUT).expect("status in time") {
                PtyEvent::Status(status) => break status,
                PtyEvent::Exit(_) => panic!("exited without a status"),
                PtyEvent::Output(_) => {}
            }
        };

        assert_eq!(status.state, brachio_core::status::AgentState::Working);
    }

    #[test]
    fn answers_a_program_status_query() {
        let manager = Arc::new(SessionManager::default());
        // Ask, then read the first bytes of the answer from the terminal input.
        let script = spec(
            "/bin/sh",
            &[
                "-c",
                r"stty raw -echo; printf '\033]7501;?\033\\'; dd bs=1 count=8 2>/dev/null | od -An -c",
            ],
        );
        let (_, events) = spawn_collecting(&manager, &script);

        let (output, _) = wait_for_exit(&events);

        assert!(
            output.contains("7   5   0   1"),
            "no answer read: {output:?}"
        );
    }
}
