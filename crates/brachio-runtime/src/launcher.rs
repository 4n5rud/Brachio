//! Starting programs: the command line they get, and the PTY events they produce.

use std::env;
use std::num::NonZeroU16;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, Weak};

use anyhow::{bail, Context, Result};
use brachio_core::status::AgentState;
use brachio_pty::{EventSink, PtyEvent, SessionManager, SpawnSpec};
use brachio_runtime::wire::{ResponseResult, StatusInfo};
use tokio::sync::watch;

use crate::runtime::Runtime;
use crate::screen::{COLS, ROWS};
use crate::session::{lock, Session, StartupOutput};

impl Runtime {
    /// Start the program for `key` unless it already runs, in which case describe that one.
    pub(crate) fn launch(&self, key: String, mut spec: SpawnSpec) -> Result<ResponseResult> {
        spec.cwd = spec
            .cwd
            .canonicalize()
            .context("resolve working directory")?;
        if !spec.cwd.is_dir() || key.is_empty() || spec.program.is_empty() {
            bail!("a session key, a program and a working directory are required");
        }
        let _reservation = self.reserve_launch(&key)?;
        {
            let mut sessions = lock(&self.sessions)?;
            self.keep_ended_programs(&mut sessions)?;
            if let Some(existing) = sessions.get(&key) {
                if existing.cwd != spec.cwd {
                    bail!("session key already belongs to a different working directory");
                }
                return existing.describe(None);
            }
            if self.draining.load(Ordering::SeqCst) {
                bail!("runtime is draining and takes no new terminals");
            }
        }
        let session = self.start_program(&spec)?;
        let id = session.id;
        let result = (|| {
            let mut sessions = lock(&self.sessions)?;
            self.forget(&key)?;
            let info = session.describe(None)?;
            sessions.insert(key.clone(), session);
            Ok(info)
        })();
        if result.is_err() {
            let _ = self.manager.kill(id);
        }
        result
    }

    fn reserve_launch<'a>(&'a self, key: &'a str) -> Result<LaunchReservation<'a>> {
        let pending = lock(&self.pending_launches)?;
        let mut pending = self
            .launch_finished
            .wait_while(pending, |pending| pending.contains(key))
            .map_err(|_| anyhow::anyhow!("runtime launch lock poisoned"))?;
        pending.insert(key.to_owned());
        Ok(LaunchReservation { runtime: self, key })
    }

    fn start_program(&self, spec: &SpawnSpec) -> Result<Session> {
        let mut initial = StartupOutput::default();
        initial.screen.resize(
            NonZeroU16::new(spec.rows).unwrap_or(ROWS),
            NonZeroU16::new(spec.cols).unwrap_or(COLS),
        );
        let output = Arc::new(Mutex::new(initial));
        let (updates, receiver) = watch::channel(());
        let sink = program_events(&output, updates, Arc::downgrade(&self.manager));
        let id = self.manager.spawn(spec, sink)?;
        let result = self.finish_start(spec, output, receiver, id);
        if result.is_err() {
            let _ = self.manager.kill(id);
        }
        result
    }

    fn finish_start(
        &self,
        spec: &SpawnSpec,
        output: Arc<Mutex<StartupOutput>>,
        receiver: watch::Receiver<()>,
        id: brachio_pty::SessionId,
    ) -> Result<Session> {
        let reply = {
            let mut state = lock(&output)?;
            state.id = Some(id);
            state.reply_if_needed()
        };
        if let Some((_, bytes)) = reply {
            self.manager.write(id, &bytes)?;
        }
        let pid = self.manager.process_id(id)?.context("read shell PID")?;
        Ok(Session {
            id,
            pid,
            cwd: spec.cwd.clone(),
            output,
            updates: receiver,
        })
    }
}

struct LaunchReservation<'a> {
    runtime: &'a Runtime,
    key: &'a str,
}

impl Drop for LaunchReservation<'_> {
    fn drop(&mut self) {
        if let Ok(mut pending) = lock(&self.runtime.pending_launches) {
            pending.remove(self.key);
            self.runtime.launch_finished.notify_all();
        }
    }
}

/// Record what a program does into `output` and wake everyone following it.
fn program_events(
    output: &Arc<Mutex<StartupOutput>>,
    updates: watch::Sender<()>,
    manager: Weak<SessionManager>,
) -> EventSink {
    let received = Arc::clone(output);
    Arc::new(move |event| match event {
        PtyEvent::Output(bytes) => {
            let reply = lock(&received)
                .ok()
                .and_then(|mut state| state.record(&bytes));
            updates.send_replace(());
            if let (Some((id, bytes)), Some(manager)) = (reply, manager.upgrade()) {
                let _ = manager.write(id, &bytes);
            }
        }
        PtyEvent::Exit(code) => {
            if let Ok(mut state) = lock(&received) {
                state.exited = true;
                state.exit_code = code;
            }
            updates.send_replace(());
        }
        PtyEvent::Status(report) => {
            if let Ok(mut state) = lock(&received) {
                state.status = (report.state != AgentState::Unknown).then_some(StatusInfo {
                    state: report.state,
                    message: report.message,
                });
            }
            updates.send_replace(());
        }
    })
}

/// Split `NAME=value` entries into pairs.
pub(crate) fn parse_env(entries: Vec<String>) -> Result<Vec<(String, String)>> {
    entries
        .into_iter()
        .map(|entry| {
            entry
                .split_once('=')
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .filter(|(name, _)| !name.is_empty())
                .with_context(|| format!("environment entry must look like NAME=value: {entry}"))
        })
        .collect()
}

#[cfg(windows)]
pub(crate) fn shell_command() -> (String, Vec<String>) {
    let program = env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned());
    (program, vec!["/d".to_owned()])
}

#[cfg(unix)]
pub(crate) fn shell_command() -> (String, Vec<String>) {
    let program = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    (program, Vec::new())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_pending_launch_only_serializes_its_own_key_and_keeps_a_draining_runtime_alive() {
        let runtime = Runtime::new(None);
        let first = runtime.reserve_launch("first").expect("reserve first");
        let other = runtime
            .reserve_launch("other")
            .expect("reserve independent key");
        runtime.draining.store(true, Ordering::SeqCst);
        assert!(!runtime.finished());
        drop(other);
        std::thread::scope(|scope| {
            let (locked, notification) = mpsc::channel();
            let runtime = &runtime;
            scope.spawn(move || {
                let _next = runtime.reserve_launch("first").expect("reserve same key");
                locked.send(()).expect("send acquisition");
            });
            assert_eq!(
                notification.recv_timeout(Duration::from_millis(25)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            drop(first);
            notification
                .recv_timeout(Duration::from_secs(2))
                .expect("same key unblocked");
        });
        assert!(runtime.finished());
    }
}
