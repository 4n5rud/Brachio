//! Terminals hosted by the independent runtime: they outlive the window and can be re-attached.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use brachio_core::status::AgentState;
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

/// A terminal this app is attached to, and the runtime that hosts it: after an update that may be
/// an older one than the app would start now.
struct Attached {
    key: String,
    client: Client,
    pump: JoinHandle<()>,
}

/// App state for runtime-hosted terminals. An attachment ID belongs to the app,
/// because different runtime generations can issue the same process-local ID.
#[derive(Default)]
pub struct RuntimeTerminals {
    client: OnceCell<Client>,
    restart: AsyncMutex<()>,
    attached: Mutex<HashMap<String, Attached>>,
}

impl RuntimeTerminals {
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
    ) -> Result<Option<(String, bool)>> {
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
        let id = self.register_attachment(key, client, pump)?;
        Ok(Some((id, attached)))
    }

    fn register_attachment(
        &self,
        key: String,
        client: Client,
        pump: JoinHandle<()>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut attached = self
            .attached
            .lock()
            .map_err(|_| anyhow!("terminal list lock poisoned"))?;
        attached.retain(|_, entry| {
            if entry.key != key {
                return true;
            }
            entry.pump.abort();
            false
        });
        attached.insert(id.clone(), Attached { key, client, pump });
        Ok(id)
    }

    /// Sends input to the attached terminal identified by the app.
    pub async fn write(&self, id: &str, data: String) -> Result<()> {
        let (client, key) = self.target(id)?;
        client.call(Action::Write { key, text: data }).await?;
        Ok(())
    }

    /// Resizes an attached terminal.
    pub async fn resize(&self, id: &str, rows: u16, cols: u16) -> Result<()> {
        let (client, key) = self.target(id)?;
        client.call(Action::Resize { key, rows, cols }).await?;
        Ok(())
    }

    /// Ends the program on purpose.
    pub async fn stop(&self, id: &str) -> Result<()> {
        let (client, key) = self.target(id)?;
        let stopped = client.call(Action::Stop { key }).await;
        self.drop_attachment(id);
        stopped?;
        Ok(())
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
                // Only a confirmed-dead endpoint is safe to restart. The connector
                // rechecks it under this lock so simultaneous reopens share one start.
                let _restart = self.restart.lock().await;
                let current = runtime_client::connect().await?;
                Ok(locate(&current, key).await?)
            }
            result => Ok(result?),
        }
    }

    fn target(&self, id: &str) -> Result<(Client, String)> {
        let attached = self
            .attached
            .lock()
            .map_err(|_| anyhow!("terminal list lock poisoned"))?;
        let found = attached
            .get(id)
            .ok_or_else(|| anyhow!("terminal attachment is no longer active"))?;
        Ok((found.client.clone(), found.key.clone()))
    }

    fn drop_attachment(&self, id: &str) {
        let removed = self.attached.lock().ok().and_then(|mut map| map.remove(id));
        if let Some(attached) = removed {
            attached.pump.abort();
        }
    }
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_missing_current_runtime_is_distinct_from_a_missing_terminal() {
        let client = Client::new(&format!("brachio-missing-runtime-{}", uuid::Uuid::new_v4()));
        assert!(matches!(
            locate(&client, "test:agent").await,
            Err(ClientError::NotRunning)
        ));
    }

    #[tokio::test]
    async fn attachment_handles_keep_runtime_routes_distinct_and_expire_on_reattach() {
        let terminals = RuntimeTerminals::default();
        let register = |key: &str, endpoint: &str| {
            terminals
                .register_attachment(key.into(), Client::new(endpoint), spawn(async {}))
                .expect("register attachment")
        };

        let old = register("first:agent", "runtime-old");
        let current = register("second:agent", "runtime-current");
        assert_ne!(old, current);
        assert_eq!(terminals.target(&old).expect("old route").1, "first:agent");
        assert_eq!(
            terminals.target(&current).expect("new route").1,
            "second:agent"
        );

        let reopened = register("first:agent", "runtime-current");
        assert_ne!(reopened, old);
        assert!(terminals.target(&old).is_err());
        assert_eq!(
            terminals.target(&reopened).expect("replacement route").1,
            "first:agent"
        );
        assert!(terminals.target(&current).is_ok());
    }
}
