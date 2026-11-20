//! Terminals hosted by the independent runtime: they outlive the window and can be re-attached.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use brachio_core::status::AgentState;
use brachio_pty::SessionId;
use brachio_runtime::client::{Client, ClientError, Following};
use brachio_runtime::wire::{Action, Reply, ResponseResult, StatusInfo};
use tauri::async_runtime::{spawn, JoinHandle};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::{Mutex as AsyncMutex, OnceCell};

use crate::runtime_client;
use crate::terminal::{send, send_message, ChannelMessage};
use crate::terminal_status::TerminalStatus;

/// How often a frame that is not yet at a replay boundary is retried.
const FRAME_ATTEMPTS: u32 = 5;
const FRAME_RETRY: Duration = Duration::from_millis(100);

/// What to run for a terminal key.
pub struct Launch {
    /// Directory the program starts in.
    pub cwd: PathBuf,
    /// Program to run.
    pub program: String,
    /// Arguments, one per element.
    pub args: Vec<String>,
    /// `NAME=value` entries.
    pub env: Vec<String>,
    /// Initial height in rows.
    pub rows: u16,
    /// Initial width in columns.
    pub cols: u16,
}

/// Handles of runtime-hosted terminals start here, far above the small numbers the in-app terminal
/// manager hands out. Each runtime numbers its sessions from 1 on its own, and an old runtime
/// can run beside the current one, so a runtime's own session id cannot name a terminal here.
const HOSTED_HANDLE_BASE: u64 = 1 << 40;

/// A terminal this app is attached to, and the runtime that hosts it: after an update that may be
/// an older one than the app would start now.
struct Attached {
    key: String,
    client: Client,
    pump: JoinHandle<()>,
}

/// App state for runtime-hosted terminals. Whether the runtime is used is decided once, so
/// terminals of the two kinds never share the session id space.
#[derive(Default)]
pub struct RuntimeTerminals {
    client: OnceCell<Client>,
    restart: AsyncMutex<()>,
    attached: Mutex<HashMap<SessionId, Attached>>,
    handles: AtomicU64,
}

impl RuntimeTerminals {
    /// Checks whether a key can be reattached without starting another process.
    pub async fn has_session(&self, key: &str) -> Result<bool> {
        let Some(client) = self.client().await? else {
            return Ok(false);
        };
        let (_, existing) = self.locate_with_restart(&client, key).await?;
        Ok(existing.is_some())
    }

    /// Keys of the task's terminals whose program is still running, in the current runtime or an
    /// older one that is still up. The runtime cannot list sessions, so the task's two fixed
    /// terminals and its registered second agent tab are asked for by name. A runtime that is not
    /// running hosts nothing. When the runtime is not used the in-app manager hosts the
    /// terminals, and it tracks sessions by handle, not key, so none can be matched to a task.
    ///
    /// # Errors
    /// Fails when a runtime cannot be asked, so the caller can refuse rather than guess.
    pub async fn live_keys_for_task(
        &self,
        task_id: &str,
        second_agent_tab: Option<&str>,
    ) -> Result<Vec<String>> {
        let Some(current) = self.client().await? else {
            return Ok(Vec::new());
        };
        let mut clients = vec![current];
        clients.extend(runtime_client::older_clients().await);
        let mut live = Vec::new();
        for key in task_terminal_keys(task_id, second_agent_tab) {
            for client in &clients {
                let reply = client.call(Action::Inspect { key: key.clone() }).await;
                match reply {
                    Ok(Reply {
                        result: ResponseResult::Success { session, .. },
                        ..
                    }) if !session.exited => {
                        live.push(key.clone());
                        break;
                    }
                    Ok(_) | Err(ClientError::Refused(_) | ClientError::NotRunning) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(live)
    }

    /// Attach to the terminal for `key`, starting it when it does not exist. When the shell
    /// is gone but its last screen was kept, that screen is shown first.
    /// Returns `None` when the runtime is not used, so the caller can host it in the app.
    ///
    /// # Errors
    /// Fails when the runtime is used but cannot be reached or refuses to start the program.
    pub async fn open(
        &self,
        key: String,
        launch: Launch,
        channel: Channel<InvokeResponseBody>,
        agent: bool,
    ) -> Result<Option<(SessionId, bool)>> {
        let Some(current) = self.client().await? else {
            return Ok(None);
        };
        let (client, existing) = self.locate_with_restart(&current, &key).await?;
        let attached = existing.is_some();
        if !attached {
            show_last_screen(&client, &key, &channel).await;
        }
        let reply = match existing {
            Some(reply) => reply,
            None => client.call(launch_action(&key, launch)).await?,
        };
        let ResponseResult::Success { .. } = reply.result else {
            bail!("the terminal runtime sent an unexpected reply");
        };
        let (first, following) = if attached {
            open_frame(&client, &key).await?
        } else {
            client
                .follow(Action::Follow {
                    key: key.clone(),
                    after: 0,
                })
                .await?
        };
        let pump = spawn(pump(first, following, channel, agent));
        let id = self.new_handle();
        let previous = self
            .attached
            .lock()
            .map_err(|_| anyhow!("terminal list lock poisoned"))?
            .insert(id, Attached { key, client, pump });
        if let Some(previous) = previous {
            previous.pump.abort();
        }
        Ok(Some((id, attached)))
    }

    /// Sends input to an attached terminal. `None` when `id` is not runtime-hosted.
    pub async fn write(&self, id: SessionId, data: String) -> Option<Result<()>> {
        let (client, key) = self.target(id).await?;
        Some(
            client
                .call(Action::Write { key, text: data })
                .await
                .map(|_| ())
                .map_err(Into::into),
        )
    }

    /// Resizes an attached terminal. `None` when `id` is not runtime-hosted.
    pub async fn resize(&self, id: SessionId, rows: u16, cols: u16) -> Option<Result<()>> {
        let (client, key) = self.target(id).await?;
        Some(
            client
                .call(Action::Resize { key, rows, cols })
                .await
                .map(|_| ())
                .map_err(Into::into),
        )
    }

    /// Ends the program on purpose. `None` when `id` is not runtime-hosted.
    pub async fn stop(&self, id: SessionId) -> Option<Result<()>> {
        let (client, key) = self.target(id).await?;
        let stopped = client.call(Action::Stop { key }).await;
        self.drop_attachment(id);
        Some(stopped.map(|_| ()).map_err(Into::into))
    }

    /// The runtime client, connecting on first use. `None` when the runtime is not used. A failed
    /// connection is not remembered, so the next open tries again.
    async fn client(&self) -> Result<Option<Client>> {
        if !runtime_client::enabled() {
            return Ok(None);
        }
        let client = self.client.get_or_try_init(runtime_client::connect).await?;
        Ok(Some(client.clone()))
    }

    async fn locate_with_restart(
        &self,
        current: &Client,
        key: &str,
    ) -> Result<(Client, Option<Reply>)> {
        match locate(current, key).await {
            Err(ClientError::NotRunning) => {
                // The connector rechecks the endpoint under this lock so concurrent reopens
                // share one start. Only a confirmed missing endpoint is safe to restart.
                let _restart = self.restart.lock().await;
                let current = runtime_client::connect().await?;
                Ok(locate(&current, key).await?)
            }
            result => Ok(result?),
        }
    }

    /// A handle for a newly attached terminal, unique in this app whichever runtime hosts it.
    fn new_handle(&self) -> SessionId {
        SessionId::from_raw(HOSTED_HANDLE_BASE + self.handles.fetch_add(1, Ordering::Relaxed))
    }

    async fn target(&self, id: SessionId) -> Option<(Client, String)> {
        let attached = self.attached.lock().ok()?;
        let found = attached.get(&id)?;
        Some((found.client.clone(), found.key.clone()))
    }

    fn drop_attachment(&self, id: SessionId) {
        let removed = self
            .attached
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&id));
        if let Some(attached) = removed {
            attached.pump.abort();
        }
    }
}

/// The terminal keys a task can have, as the UI and `terminal_authorization` name them.
fn task_terminal_keys(task_id: &str, second_agent_tab: Option<&str>) -> Vec<String> {
    let mut keys = vec![format!("{task_id}:agent"), format!("{task_id}:shell")];
    keys.extend(second_agent_tab.map(|tab| format!("{task_id}:agent-{tab}")));
    keys
}

/// Finds the runtime that already hosts `key`: the current one, else an older one that is still
/// running. Only "no such terminal" counts as absent; any other failure stops the open, because
/// starting a second program for the same key would be worse than not opening one.
async fn locate(current: &Client, key: &str) -> Result<(Client, Option<Reply>), ClientError> {
    let inspect = || Action::Inspect {
        key: key.to_owned(),
    };
    match current.call(inspect()).await {
        Ok(reply) => return Ok((current.clone(), Some(reply))),
        Err(ClientError::Refused(_)) => {}
        Err(error) => return Err(error),
    }
    for old in runtime_client::older_clients().await {
        match old.call(inspect()).await {
            Ok(reply) => return Ok((old, Some(reply))),
            Err(ClientError::Refused(_) | ClientError::NotRunning) => {}
            Err(error) => return Err(error),
        }
    }
    Ok((current.clone(), None))
}

fn launch_action(key: &str, launch: Launch) -> Action {
    Action::Launch {
        key: key.to_owned(),
        cwd: launch.cwd,
        program: launch.program,
        args: launch.args,
        env: launch.env,
        rows: launch.rows,
        cols: launch.cols,
    }
}

/// Show what a shell that is gone last displayed, so a restart does not look like a blank slate.
async fn show_last_screen(client: &Client, key: &str, channel: &Channel<InvokeResponseBody>) {
    let recovered = client
        .call(Action::Recover {
            key: key.to_owned(),
        })
        .await;
    if let Ok(Reply {
        result: ResponseResult::Recovered { screen, .. },
        ..
    }) = recovered
    {
        send(channel, InvokeResponseBody::Raw(screen.ansi.into_bytes()));
        send_message(channel, &ChannelMessage::Restored { restored: true });
    }
}

/// Follow from the settled frame, or from the retained history when no frame settles.
async fn open_frame(client: &Client, key: &str) -> Result<(Reply, Following), ClientError> {
    for _ in 0..FRAME_ATTEMPTS {
        match client
            .follow(Action::FollowFrame {
                key: key.to_owned(),
            })
            .await
        {
            Err(ClientError::Refused(_)) => tokio::time::sleep(FRAME_RETRY).await,
            other => return other,
        }
    }
    client
        .follow(Action::Follow {
            key: key.to_owned(),
            after: 0,
        })
        .await
}

/// Forward everything the runtime reports to the web view until the program ends.
async fn pump(
    first: Reply,
    mut following: Following,
    channel: Channel<InvokeResponseBody>,
    agent: bool,
) {
    let mut last_status = None;
    let status = agent.then(|| TerminalStatus::new(channel.clone()));
    let mut live = false;
    let mut next = Some(first);
    loop {
        if let Some(reply) = next.take() {
            if deliver(reply, &mut last_status, &channel, status.as_ref(), live) {
                return;
            }
            live = true;
        }
        match following.next().await {
            Ok(Some(reply)) => next = Some(reply),
            Ok(None) | Err(_) => {
                send_message(&channel, &ChannelMessage::Detached { detached: true });
                return;
            }
        }
    }
}

/// Send one reply on; `true` once the program has ended and its output is fully sent.
fn deliver(
    reply: Reply,
    last_status: &mut Option<StatusInfo>,
    channel: &Channel<InvokeResponseBody>,
    status: Option<&TerminalStatus>,
    live: bool,
) -> bool {
    let (session, bytes, drained) = match reply.result {
        ResponseResult::Snapshot { session, screen } => (session, screen.ansi.into_bytes(), true),
        ResponseResult::Success { session, output } => match output {
            Some(page) => {
                let drained = page.next == page.end;
                (session, page.bytes, drained)
            }
            None => (session, Vec::new(), true),
        },
        _ => return false,
    };
    if !bytes.is_empty() {
        if let Some(status) = status.filter(|_| live) {
            status.activity();
        }
        send(channel, InvokeResponseBody::Raw(bytes));
    }
    if session.status != *last_status {
        let (state, message) = session
            .status
            .clone()
            .map_or((AgentState::Unknown, None), |status| {
                (status.state, status.message)
            });
        if let Some(status) = status {
            status.osc(state, message);
        }
        *last_status = session.status.clone();
    }
    if session.exited && drained {
        send_message(
            channel,
            &ChannelMessage::Exit {
                exit_code: session.exit_code,
            },
        );
        return true;
    }
    false
}

#[cfg(test)]
mod handle_tests {
    use brachio_pty::SessionId;

    use super::{locate, Client, ClientError, RuntimeTerminals, HOSTED_HANDLE_BASE};

    #[tokio::test]
    async fn a_missing_current_runtime_is_distinct_from_a_missing_terminal() {
        let client = Client::new(&format!("brachio-missing-runtime-{}", uuid::Uuid::new_v4()));
        assert!(matches!(
            locate(&client, "test:agent").await,
            Err(ClientError::NotRunning)
        ));
    }

    #[test]
    fn hosted_terminals_get_distinct_handles_that_cannot_equal_an_in_app_session_id() {
        let terminals = RuntimeTerminals::default();
        let first = terminals.new_handle();
        let second = terminals.new_handle();
        assert_ne!(first, second, "two runtimes can both have a session 1");
        assert!(first > SessionId::from_raw(HOSTED_HANDLE_BASE - 1));
        assert!(first > SessionId::from_raw(1_000_000));
    }
}

#[cfg(test)]
mod tests {
    use super::task_terminal_keys;

    #[test]
    fn a_task_names_its_agent_shell_and_second_agent_terminals() {
        assert_eq!(
            task_terminal_keys("t1", Some("abc")),
            ["t1:agent", "t1:shell", "t1:agent-abc"]
        );
        assert_eq!(task_terminal_keys("t1", None), ["t1:agent", "t1:shell"]);
    }
}
