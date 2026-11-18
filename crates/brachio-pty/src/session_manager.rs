//! Owns every live PTY and streams its output to the UI.

use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
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

        let (chunk_sender, chunk_receiver) = mpsc::channel();
        thread::Builder::new()
            .name(format!("pty-read-{id}"))
            .spawn(move || read_until_closed(reader, &chunk_sender))
            .context("start pty reader")?;

        let manager = Arc::clone(self);
        let batch_config = self.batch_config;
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
        Ok(id)
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

fn read_until_closed(mut reader: Box<dyn Read + Send>, chunks: &Sender<Vec<u8>>) {
    let mut buffer = vec![0u8; 16 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
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
    while let Ok(chunk) = chunks.try_recv() {
        batcher.push(&chunk, 0);
    }
    if let Some(frame) = batcher.tick(u64::MAX) {
        sink(PtyEvent::Output(frame));
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
}
