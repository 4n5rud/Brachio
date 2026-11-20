//! The runtime: every program it hosts, and how requests act on them.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use anyhow::{bail, Context, Result};
use brachio_pty::{SessionManager, SpawnSpec};
use brachio_runtime::protocol;
use brachio_runtime::wire::{Action, Request, ResponseResult, RuntimeInfo};
use uuid::Uuid;

use crate::journal::{Journal, SessionRecord};
use crate::launcher::{parse_env, shell_command};
use crate::screen::{COLS, ROWS};
use crate::session::{get, lock, ScreenMark, Session};

pub(crate) struct Runtime {
    pub(crate) instance: String,
    pub(crate) manager: Arc<SessionManager>,
    pub(crate) sessions: Mutex<HashMap<String, Session>>,
    pub(crate) pending_launches: Mutex<HashSet<String>>,
    pub(crate) launch_finished: Condvar,
    pub(crate) draining: AtomicBool,
    pub(crate) journal: Option<Journal>,
    /// Shells that are gone but whose last screen can still be read.
    pub(crate) lost: Mutex<HashMap<String, SessionRecord>>,
    /// Keys the user stopped on purpose; their records are deleted, not kept.
    pub(crate) stopped: Mutex<HashSet<String>>,
    /// Which screen revision the journal already holds for each key.
    pub(crate) saved: Mutex<HashMap<String, ScreenMark>>,
}

impl Runtime {
    pub(crate) fn new(journal: Option<Journal>) -> Self {
        let lost = journal.as_ref().map(Journal::read_lost).unwrap_or_default();
        Self {
            instance: Uuid::new_v4().to_string(),
            manager: Arc::new(SessionManager::default()),
            sessions: Mutex::default(),
            pending_launches: Mutex::default(),
            launch_finished: Condvar::new(),
            draining: AtomicBool::new(false),
            journal,
            lost: Mutex::new(lost),
            stopped: Mutex::default(),
            saved: Mutex::default(),
        }
    }

    /// A draining runtime with no shells left has nothing more to serve.
    pub(crate) fn finished(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
            && self.pending_launches.lock().is_ok_and(|launches| {
                launches.is_empty() && self.manager.ids().is_ok_and(|ids| ids.is_empty())
            })
    }

    pub(crate) fn hello(&self) -> ResponseResult {
        ResponseResult::Hello {
            runtime: RuntimeInfo {
                pid: std::process::id(),
                oldest_version: protocol::OLDEST,
                current_version: protocol::CURRENT,
                draining: self.draining.load(Ordering::SeqCst),
            },
        }
    }

    pub(crate) fn execute(&self, request: Request) -> Result<ResponseResult> {
        match request.action {
            Action::Hello => Ok(self.hello()),
            Action::Drain => {
                self.draining.store(true, Ordering::SeqCst);
                Ok(self.hello())
            }
            Action::Ensure { key, cwd } => {
                let (program, args) = shell_command();
                let spec = SpawnSpec {
                    program,
                    args,
                    cwd,
                    env: Vec::new(),
                    rows: ROWS.get(),
                    cols: COLS.get(),
                };
                self.launch(key, spec)
            }
            Action::Launch {
                key,
                cwd,
                program,
                args,
                env,
                rows,
                cols,
            } => {
                let spec = SpawnSpec {
                    program,
                    args,
                    cwd,
                    env: parse_env(env)?,
                    rows: rows.max(1),
                    cols: cols.max(1),
                };
                self.launch(key, spec)
            }
            action => {
                let mut sessions = lock(&self.sessions)?;
                self.keep_ended_programs(&mut sessions)?;
                match action {
                    Action::Lost => self.list_lost(),
                    Action::Recover { key } => self.recover(&key),
                    Action::Forget { key } => {
                        self.forget(&key)?;
                        Ok(self.hello())
                    }
                    action => self.act_on_session(&sessions, action),
                }
            }
        }
    }

    /// Requests that address one running program by its key.
    fn act_on_session(
        &self,
        sessions: &HashMap<String, Session>,
        action: Action,
    ) -> Result<ResponseResult> {
        match action {
            Action::Inspect { key } => get(sessions, &key)?.describe(None),
            Action::Read { key, after } => get(sessions, &key)?.describe(Some(after)),
            Action::Snapshot { key } => get(sessions, &key)?.snapshot(),
            Action::Resize { key, rows, cols } => self.resize(get(sessions, &key)?, rows, cols),
            Action::Write { key, text } => {
                let session = get(sessions, &key)?;
                self.manager.write(session.id, text.as_bytes())?;
                session.describe(None)
            }
            Action::Stop { key } => {
                let session = get(sessions, &key)?;
                lock(&self.stopped)?.insert(key.clone());
                if let Err(error) = self
                    .forget(&key)
                    .and_then(|()| self.manager.kill(session.id))
                {
                    lock(&self.stopped)?.remove(&key);
                    return Err(error);
                }
                session.describe(None)
            }
            Action::Follow { .. } | Action::FollowFrame { .. } => {
                bail!("follow requires a streaming response")
            }
            _ => bail!("this request does not address a running program"),
        }
    }

    fn resize(&self, session: &Session, rows: u16, cols: u16) -> Result<ResponseResult> {
        let rows = NonZeroU16::new(rows).context("rows must be at least 1")?;
        let cols = NonZeroU16::new(cols).context("columns must be at least 1")?;
        self.manager.resize(session.id, rows.get(), cols.get())?;
        lock(&session.output)?.screen.resize(rows, cols);
        session.describe(None)
    }
}
