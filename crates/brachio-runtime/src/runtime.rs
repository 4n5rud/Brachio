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
    operations: Mutex<HashMap<String, (String, Option<ResponseResult>)>>,
    operations_finished: Condvar,
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
            operations: Mutex::default(),
            operations_finished: Condvar::new(),
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
            Action::LaunchOutcome {
                instance,
                operation_id,
            } => {
                if instance != self.instance {
                    bail!("runtime instance changed; launch outcome is unknown");
                }
                let operations = lock(&self.operations)?;
                match operations.get(&operation_id) {
                    Some((_, Some(result))) => Ok(ResponseResult::LaunchCompleted {
                        launch_outcome: Box::new(result.clone()),
                    }),
                    Some((_, None)) => Ok(ResponseResult::LaunchPending {
                        launch_pending: true,
                    }),
                    None => bail!("launch operation was not observed by this runtime"),
                }
            }
            Action::LaunchOnce {
                instance,
                operation_id,
                key,
                cwd,
                program,
                args,
                env,
                rows,
                cols,
            } => {
                if instance != self.instance {
                    bail!("runtime instance changed; launch outcome is unknown");
                }
                if operation_id.is_empty() || operation_id.len() > 128 {
                    bail!("invalid launch operation ID");
                }
                let arguments =
                    serde_json::to_string(&(&key, &cwd, &program, &args, &env, rows, cols))?;
                let mut operations = lock(&self.operations)?;
                loop {
                    match operations.get(&operation_id) {
                        Some((saved, _)) if saved != &arguments => {
                            bail!("launch operation ID already used with different arguments")
                        }
                        Some((_, Some(result))) => return Ok(result.clone()),
                        Some((_, None)) => {
                            operations = self
                                .operations_finished
                                .wait(operations)
                                .map_err(|_| anyhow::anyhow!("launch operations lock poisoned"))?;
                        }
                        None => {
                            let stored_bytes: usize = operations
                                .iter()
                                .map(|(id, (saved, _))| id.len() + saved.len())
                                .sum();
                            if operations.len() >= 4096
                                || stored_bytes + operation_id.len() + arguments.len() > 1024 * 1024
                            {
                                bail!("launch operation history is full");
                            }
                            operations.insert(operation_id.clone(), (arguments, None));
                            break;
                        }
                    }
                }
                drop(operations);
                let result = (|| {
                    let spec = SpawnSpec {
                        program,
                        args,
                        cwd,
                        env: parse_env(env)?,
                        rows: rows.max(1),
                        cols: cols.max(1),
                    };
                    self.launch(key, spec)
                })()
                .unwrap_or_else(|error| ResponseResult::Error {
                    error: format!("{error:#}"),
                });
                let mut operations = lock(&self.operations)?;
                if let Some((_, saved)) = operations.get_mut(&operation_id) {
                    *saved = Some(result.clone());
                }
                self.operations_finished.notify_all();
                Ok(result)
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
            Action::StopGuarded {
                key,
                instance,
                session_id,
            } => {
                let session = get(sessions, &key)?;
                if instance != self.instance
                    || session.id != brachio_pty::SessionId::from_raw(session_id)
                {
                    bail!("terminal session changed before stop");
                }
                self.act_on_session(sessions, Action::Stop { key })
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

#[cfg(test)]
mod operation_tests {
    use super::*;

    fn launch(instance: String, operation_id: &str, program: &str) -> Request {
        Request {
            version: 6,
            action: Action::LaunchOnce {
                instance,
                operation_id: operation_id.to_owned(),
                key: "test".to_owned(),
                cwd: std::env::temp_dir(),
                program: program.to_owned(),
                args: Vec::new(),
                env: Vec::new(),
                rows: 24,
                cols: 80,
            },
        }
    }

    #[test]
    fn completed_failure_is_retained_and_ids_are_bound_to_arguments_and_instance() {
        let runtime = Runtime::new(None);
        let request = launch(runtime.instance.clone(), "operation", "");
        let first = runtime.execute(request.clone()).expect("stored failure");
        assert!(matches!(first, ResponseResult::Error { .. }));
        let second = runtime.execute(request).expect("replayed failure");
        assert_eq!(
            serde_json::to_string(&first).expect("json"),
            serde_json::to_string(&second).expect("json")
        );
        assert!(runtime
            .execute(launch(runtime.instance.clone(), "operation", "other"))
            .is_err());
        assert!(runtime
            .execute(launch("other instance".to_owned(), "operation", ""))
            .is_err());
    }
    #[cfg(windows)]
    #[test]
    fn same_operation_reuses_original_result_after_the_process_exits() {
        let runtime = Runtime::new(None);
        let mut request = launch(runtime.instance.clone(), "completed", "cmd.exe");
        if let Action::LaunchOnce { args, .. } = &mut request.action {
            *args = vec!["/d".into(), "/c".into(), "exit".into()];
        }
        let first = runtime.execute(request.clone()).expect("launch");
        let ResponseResult::Success { session, .. } = first else {
            panic!("launch failed");
        };
        runtime.manager.kill(session.session_id).ok();
        let second = runtime.execute(request).expect("replay");
        let ResponseResult::Success { session: again, .. } = second else {
            panic!("replay failed");
        };
        assert_eq!(session.session_id, again.session_id);
    }

    #[test]
    fn a_full_history_refuses_a_new_id_before_starting_a_process() {
        let runtime = Runtime::new(None);
        {
            let mut history = runtime.operations.lock().expect("history");
            for n in 0..4096 {
                history.insert(
                    n.to_string(),
                    (
                        String::new(),
                        Some(ResponseResult::Error {
                            error: "done".into(),
                        }),
                    ),
                );
            }
        }
        assert!(runtime
            .execute(launch(runtime.instance.clone(), "new", "anything"))
            .is_err());
        assert!(runtime.manager.ids().expect("sessions").is_empty());
    }
    #[test]
    fn concurrent_identical_operations_share_one_result() {
        let runtime = Runtime::new(None);
        let request = launch(runtime.instance.clone(), "concurrent", "");
        let barrier = std::sync::Barrier::new(3);
        std::thread::scope(|scope| {
            let first_request = request.clone();
            let first = scope.spawn(|| {
                barrier.wait();
                runtime.execute(first_request).expect("first")
            });
            let second = scope.spawn(|| {
                barrier.wait();
                runtime.execute(request).expect("second")
            });
            barrier.wait();
            assert_eq!(
                serde_json::to_string(&first.join().expect("first thread")).expect("json"),
                serde_json::to_string(&second.join().expect("second thread")).expect("json")
            );
        });
        assert_eq!(runtime.operations.lock().expect("history").len(), 1);
    }
    #[test]
    fn launch_outcome_returns_completed_pending_and_unknown_without_starting_a_process() {
        let runtime = Runtime::new(None);
        let query = |id: &str| Request {
            version: 6,
            action: Action::LaunchOutcome {
                instance: runtime.instance.clone(),
                operation_id: id.to_owned(),
            },
        };
        assert!(runtime.execute(query("absent")).is_err());
        {
            let mut operations = runtime.operations.lock().expect("operations");
            operations.insert("pending".into(), (String::new(), None));
        }
        assert!(matches!(
            runtime.execute(query("pending")).expect("pending"),
            ResponseResult::LaunchPending {
                launch_pending: true
            }
        ));
        let request = launch(runtime.instance.clone(), "finished", "");
        let original = runtime.execute(request).expect("stored outcome");
        let replay = runtime.execute(query("finished")).expect("read outcome");
        let ResponseResult::LaunchCompleted { launch_outcome } = replay else {
            panic!("expected completed outcome");
        };
        assert_eq!(
            serde_json::to_string(&original).expect("json"),
            serde_json::to_string(&launch_outcome).expect("json")
        );
        assert!(runtime.manager.ids().expect("sessions").is_empty());
    }
}
