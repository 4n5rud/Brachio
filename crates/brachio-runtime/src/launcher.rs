//! Starting programs: the command line they get, and the PTY events they produce.

use std::collections::HashMap;
use std::env;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, Weak};

use anyhow::{bail, Context, Result};
use brachio_core::status::AgentState;
use brachio_pty::{EventSink, PtyEvent, SessionManager, SpawnSpec};
use brachio_runtime::wire::{ResponseResult, StatusInfo};
use tokio::sync::watch;

use crate::runtime::Runtime;
use crate::session::{lock, Session, StartupOutput};

/// The answer to a program that asks where the cursor is, so it can start without a screen.
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";

impl Runtime {
    /// Start the program for `key` unless it already runs, in which case describe that one.
    pub(crate) fn launch(
        &self,
        sessions: &mut HashMap<String, Session>,
        key: String,
        mut spec: SpawnSpec,
    ) -> Result<ResponseResult> {
        spec.cwd = spec
            .cwd
            .canonicalize()
            .context("resolve working directory")?;
        if !spec.cwd.is_dir() || key.is_empty() || spec.program.is_empty() {
            bail!("a session key, a program and a working directory are required");
        }
        if let Some(existing) = sessions.get(&key) {
            if existing.cwd != spec.cwd {
                bail!("session key already belongs to a different working directory");
            }
            return existing.describe(None);
        }
        if self.draining.load(Ordering::SeqCst) {
            bail!("runtime is draining and takes no new terminals");
        }
        let session = self.start_program(&spec)?;
        self.forget(&key)?;
        let info = session.describe(None)?;
        sessions.insert(key, session);
        Ok(info)
    }

    fn start_program(&self, spec: &SpawnSpec) -> Result<Session> {
        let output = Arc::new(Mutex::new(StartupOutput::default()));
        let (updates, receiver) = watch::channel(());
        let sink = program_events(&output, updates, Arc::downgrade(&self.manager));
        let id = self.manager.spawn(spec, sink)?;
        let reply = {
            let mut state = lock(&output)?;
            state.id = Some(id);
            state.reply_if_needed()
        };
        if reply.is_some() {
            self.manager.write(id, CURSOR_REPLY)?;
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
            if let (Some(id), Some(manager)) = (reply, manager.upgrade()) {
                let _ = manager.write(id, CURSOR_REPLY);
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
