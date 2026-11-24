//! Terminals hosted by the independent runtime: they outlive the window and can be re-attached.

use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use brachio_core::status::AgentState;
use brachio_pty::SessionId;
use brachio_runtime::client::{Client, ClientError, Following};
use brachio_runtime::wire::{Action, Reply, ResponseResult, StatusInfo, MAX_MESSAGE};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::{spawn, JoinHandle};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::{Mutex as AsyncMutex, OnceCell};
use uuid::Uuid;

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
    instance: String,
    session_id: u64,
    client: Client,
    pump: JoinHandle<()>,
    /// Cleared while the terminal is parked: output stays in the runtime, only status flows on.
    forwarding: Arc<AtomicBool>,
}

/// What a terminal that is gone last displayed, with how it ended.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedScreen {
    /// Escape sequences that redraw the screen.
    pub ansi: String,
    /// Frame height in rows.
    pub rows: u16,
    /// Frame width in columns.
    pub cols: u16,
    /// Whether the program ended by itself rather than being lost with its runtime.
    pub exited: bool,
    /// Its exit code, when it ended.
    pub exit_code: Option<u32>,
}

fn saved_screen_from(result: ResponseResult) -> Option<SavedScreen> {
    let ResponseResult::Recovered { lost, screen } = result else {
        return None;
    };
    Some(SavedScreen {
        ansi: screen.ansi,
        rows: screen.rows,
        cols: screen.cols,
        exited: lost.exited,
        exit_code: lost.exit_code,
    })
}

/// App state for runtime-hosted terminals. Whether the runtime is used is decided once, so
/// terminals of the two kinds never share the session id space.
#[derive(Default)]
pub struct RuntimeTerminals {
    client: OnceCell<Client>,
    restart: AsyncMutex<()>,
    launch_lock: AsyncMutex<()>,
    attached: Arc<Mutex<HashMap<SessionId, Attached>>>,
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
    /// terminals and every extra tab it has had are asked for by name. A runtime that is not
    /// running hosts nothing. When the runtime is not used the in-app manager hosts the
    /// terminals, which `Terminals::live_keys_for_task` answers instead.
    ///
    /// # Errors
    /// Fails when a runtime cannot be asked, so the caller can refuse rather than guess.
    pub async fn live_keys_for_task(
        &self,
        task_id: &str,
        extra_tabs: &[String],
    ) -> Result<Vec<String>> {
        Ok(self
            .live_sessions_for_task(task_id, extra_tabs)
            .await?
            .into_iter()
            .map(|(key, _)| key)
            .collect())
    }

    /// Like `live_keys_for_task`, with the process ID of each running terminal's shell.
    ///
    /// # Errors
    /// Fails when a runtime cannot be asked.
    pub async fn live_sessions_for_task(
        &self,
        task_id: &str,
        extra_tabs: &[String],
    ) -> Result<Vec<(String, u32)>> {
        let Some(current) = self.client().await? else {
            return Ok(Vec::new());
        };
        let mut clients = vec![current];
        clients.extend(runtime_client::older_clients().await);
        let mut live = Vec::new();
        for key in task_terminal_keys(task_id, extra_tabs) {
            for client in &clients {
                let reply = client.call(Action::Inspect { key: key.clone() }).await;
                match reply {
                    Ok(Reply {
                        result: ResponseResult::Success { session, .. },
                        ..
                    }) if !session.exited => {
                        live.push((key.clone(), session.pid));
                        break;
                    }
                    Ok(_) | Err(ClientError::Refused(_) | ClientError::NotRunning) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(live)
    }

    /// The last screen of a terminal that is gone, from the current runtime or an older one that is
    /// still up. `None` when the terminal is still running, was never saved, or the runtime is
    /// not used. Reading starts nothing.
    ///
    /// # Errors
    /// Fails when the current runtime cannot be reached.
    pub async fn saved_screen(&self, key: &str) -> Result<Option<SavedScreen>> {
        let Some(current) = self.client().await? else {
            return Ok(None);
        };
        let mut clients = vec![current];
        clients.extend(runtime_client::older_clients().await);
        for client in clients {
            let reply = client
                .call(Action::Recover {
                    key: key.to_owned(),
                })
                .await;
            match reply {
                Ok(Reply { result, .. }) => {
                    if let Some(screen) = saved_screen_from(result) {
                        return Ok(Some(screen));
                    }
                }
                Err(ClientError::Refused(_) | ClientError::NotRunning) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
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
        let _launch_guard = self.launch_lock.lock().await;
        let pending = pending_launch(&key)?;
        let (client, existing, attached) = if pending.is_some() {
            (current.clone(), None, false)
        } else {
            let (client, existing) = self.locate_with_restart(&current, &key).await?;
            let attached = existing.is_some();
            (client, existing, attached)
        };
        if !attached {
            show_last_screen(&client, &key, &channel).await;
        }
        let reply = match existing {
            Some(reply) => reply,
            None => {
                let record = match pending {
                    Some(record) => record,
                    None => {
                        let hello = client.call(Action::Hello).await?;
                        let ResponseResult::Hello { runtime } = hello.result else {
                            bail!("runtime did not provide its protocol version");
                        };
                        if runtime.current_version < 6 {
                            bail!("the running terminal runtime cannot safely start a new session; update or restart the runtime");
                        }
                        let record = PendingLaunch {
                            key: key.clone(),
                            instance: hello.instance,
                            operation_id: Uuid::new_v4().to_string(),
                        };
                        let action = launch_action(&key, launch, &record);
                        ensure_request_fits(&action)?;
                        save_pending_launch(&record)?;
                        let result = match client.call(action).await {
                            Ok(result) => result,
                            Err(ClientError::Refused(message)) => {
                                remove_pending_launch(&record)?;
                                bail!("terminal launch refused: {message}");
                            }
                            Err(error) => return Err(error.into()),
                        };
                        let attached = self
                            .attach_launch_reply(key, client, result, channel, agent, false)
                            .await?;
                        remove_pending_launch(&record)?;
                        return Ok(attached);
                    }
                };
                let looked_up = client
                    .call(Action::LaunchOutcome {
                        instance: record.instance.clone(),
                        operation_id: record.operation_id.clone(),
                    })
                    .await;
                let (outcome, reply_meta) = match looked_up {
                    Ok(reply) => {
                        let result = reply.result.clone();
                        (Ok(result), Some(reply))
                    }
                    Err(error) => (Err(error), None),
                };
                let result = match reconcile_launch(&record, outcome)? {
                    Reconciled::Attach(result) => result,
                    Reconciled::NotStarted(error) => {
                        remove_pending_launch(&record)?;
                        bail!("terminal launch failed: {error}");
                    }
                };
                let result = Reply {
                    result,
                    ..reply_meta.expect("outcome came from a reply")
                };
                let attached = self
                    .attach_launch_reply(key, client, result, channel, agent, false)
                    .await?;
                remove_pending_launch(&record)?;
                return Ok(attached);
            }
        };
        drop(_launch_guard);
        self.attach_launch_reply(key, client, reply, channel, agent, attached)
            .await
    }

    async fn attach_launch_reply(
        &self,
        key: String,
        client: Client,
        reply: Reply,
        channel: Channel<InvokeResponseBody>,
        agent: bool,
        attached: bool,
    ) -> Result<Option<(SessionId, bool)>> {
        let ResponseResult::Success { session, .. } = reply.result else {
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
        let id = self.new_handle();
        let session_id = session
            .session_id
            .to_string()
            .parse()
            .map_err(|_| anyhow!("invalid runtime session ID"))?;
        let forwarding = Arc::new(AtomicBool::new(true));
        let reopen = (client.clone(), key.clone());
        self.register(
            id,
            key,
            client,
            (reply.instance, session_id),
            &forwarding,
            pump(
                reopen,
                first,
                following,
                channel,
                agent,
                Arc::clone(&forwarding),
            ),
        )?;
        Ok(Some((id, attached)))
    }

    /// Forget one unknown launch after the user explicitly confirms the duplicate risk.
    pub async fn forget_uncertain_launch(&self, key: &str, operation_id: &str) -> Result<()> {
        let _guard = self.launch_lock.lock().await;
        let record = pending_launch(key)?.ok_or_else(|| anyhow!("No pending launch exists."))?;
        if record.operation_id != operation_id {
            bail!("The pending launch changed. Reopen the terminal to check it again.");
        }
        remove_pending_launch(&record)
    }

    /// Sends input to an attached terminal. `None` when `id` is not runtime-hosted.
    pub async fn write(&self, id: SessionId, data: String) -> Option<Result<()>> {
        let (client, key) = self.target(id).await?;
        for text in input_chunks(&data) {
            if let Err(error) = client
                .call(Action::Write {
                    key: key.clone(),
                    text: text.to_owned(),
                })
                .await
            {
                return Some(Err(error.into()));
            }
        }
        Some(Ok(()))
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
        let (client, key, instance, session_id) = {
            let attached = self.attached.lock().ok()?;
            let found = attached.get(&id)?;
            (
                found.client.clone(),
                found.key.clone(),
                found.instance.clone(),
                found.session_id,
            )
        };
        let stopped = client
            .call(Action::StopGuarded {
                key,
                instance,
                session_id,
            })
            .await;
        self.drop_attachment(id);
        Some(stopped.map(|_| ()).map_err(Into::into))
    }

    /// Stops watching a terminal while its program keeps running in the runtime; opening the same
    /// key again reattaches. The shared client stays open. `None` when `id` is not runtime-hosted.
    pub fn detach(&self, id: SessionId) -> Option<Result<()>> {
        let hosted = self.attached.lock().ok()?.contains_key(&id);
        hosted.then(|| {
            self.drop_attachment(id);
            Ok(())
        })
    }

    /// Keeps a hidden terminal's status flowing without sending its output to the web view, which
    /// lets the UI unmount the terminal. Opening the same key again replaces the parked attachment.
    /// `None` when `id` is not runtime-hosted.
    pub fn park(&self, id: SessionId) -> Option<Result<()>> {
        let attached = self.attached.lock().ok()?;
        let found = attached.get(&id)?;
        found.forwarding.store(false, Ordering::Relaxed);
        Some(Ok(()))
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

    /// Runs `forward` and records the attachment under `id`; the entry goes away when `forward`
    /// ends, whether the program exited or the stream broke, without touching the program.
    fn register(
        &self,
        id: SessionId,
        key: String,
        client: Client,
        (instance, session_id): (String, u64),
        forwarding: &Arc<AtomicBool>,
        forward: impl Future<Output = ()> + Send + 'static,
    ) -> Result<()> {
        let map = Arc::clone(&self.attached);
        let mut attached = self
            .attached
            .lock()
            .map_err(|_| anyhow!("terminal list lock poisoned"))?;
        // Held until the insert so a pump that ends at once cannot remove before it is added.
        let pump = spawn(async move {
            forward.await;
            if let Ok(mut map) = map.lock() {
                map.remove(&id);
            }
        });
        let replaced: Vec<SessionId> = attached
            .iter()
            .filter(|(_, other)| other.key == key && !other.forwarding.load(Ordering::Relaxed))
            .map(|(old, _)| *old)
            .collect();
        for old in replaced {
            if let Some(parked) = attached.remove(&old) {
                parked.pump.abort();
            }
        }
        attached.insert(
            id,
            Attached {
                key,
                instance,
                session_id,
                client,
                pump,
                forwarding: Arc::clone(forwarding),
            },
        );
        Ok(())
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

/// Drops the saved screens of a deleted task from the current runtime and every older one that is
/// still up. A runtime that is not running or predates the action has none to drop, and a failure
/// here never undoes the delete.
pub async fn forget_task_screens(task_id: &str) {
    if !runtime_client::enabled() {
        return;
    }
    let mut clients = vec![Client::new(&runtime_client::endpoint())];
    clients.extend(runtime_client::older_clients().await);
    for client in clients {
        let action = Action::ForgetTask {
            prefix: format!("{task_id}:"),
        };
        match client.call(action).await {
            Ok(_) | Err(ClientError::NotRunning) => {}
            Err(ClientError::Refused(reason)) => {
                tracing::debug!(%reason, "runtime kept the deleted task's saved screens");
            }
            Err(error) => {
                tracing::warn!(%error, "could not forget the deleted task's saved screens");
            }
        }
    }
}

/// The terminal keys a task can have, as the UI and `terminal_authorization` name them.
fn task_terminal_keys(task_id: &str, extra_tabs: &[String]) -> Vec<String> {
    let mut keys = vec![format!("{task_id}:agent"), format!("{task_id}:shell")];
    for tab in extra_tabs {
        keys.push(format!("{task_id}:agent-{tab}"));
        keys.push(format!("{task_id}:shell-{tab}"));
    }
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

/// Longest text sent in one write request; a typed-in first prompt can be far larger.
const WRITE_CHUNK_BYTES: usize = 16 * 1024;

/// Split input at character boundaries so each piece fits one runtime request.
fn input_chunks(data: &str) -> impl Iterator<Item = &str> {
    let mut rest = data;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut end = rest.len().min(WRITE_CHUNK_BYTES);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let (chunk, tail) = rest.split_at(end);
        rest = tail;
        Some(chunk)
    })
}

/// Refuse a request the runtime would reject before any pending record is saved for it.
fn ensure_request_fits(action: &Action) -> Result<()> {
    // Room for the version field and the line ending around the action.
    let size = serde_json::to_vec(action)?.len() as u64 + 64;
    if size > MAX_MESSAGE {
        bail!(
            "The launch command is too large for the terminal runtime ({size} bytes, limit {MAX_MESSAGE}). Shorten the first prompt."
        );
    }
    Ok(())
}

fn launch_action(key: &str, launch: Launch, pending: &PendingLaunch) -> Action {
    Action::LaunchOnce {
        instance: pending.instance.clone(),
        operation_id: pending.operation_id.clone(),
        key: key.to_owned(),
        cwd: launch.cwd,
        program: launch.program,
        args: launch.args,
        env: launch.env,
        rows: launch.rows,
        cols: launch.cols,
    }
}

enum Reconciled {
    Attach(ResponseResult),
    NotStarted(String),
}

/// Decide what a recorded launch became; anything not provably started or failed stays uncertain.
fn reconcile_launch(
    record: &PendingLaunch,
    outcome: Result<ResponseResult, ClientError>,
) -> Result<Reconciled> {
    match outcome {
        Ok(ResponseResult::LaunchPending { .. }) => bail!("terminal launch is still in progress"),
        Ok(ResponseResult::LaunchCompleted { launch_outcome }) => match *launch_outcome {
            ResponseResult::Error { error } => Ok(Reconciled::NotStarted(error)),
            result => Ok(Reconciled::Attach(result)),
        },
        Ok(_) => bail!("terminal runtime sent an unexpected launch outcome"),
        Err(ClientError::Refused(message))
            if message.contains("launch operation was not observed")
                || message.contains("runtime instance changed")
                || message.contains("requires protocol") =>
        {
            Err(anyhow!("BRACHIO_LAUNCH_UNCERTAIN:{}", record.operation_id))
        }
        Err(error) => Err(error.into()),
    }
}

#[derive(Serialize, Deserialize)]
struct PendingLaunch {
    key: String,
    instance: String,
    operation_id: String,
}

fn pending_dir() -> Result<PathBuf> {
    Ok(brachio_runtime::paths::state_directory()?.join("pending-launches"))
}

fn pending_launch(key: &str) -> Result<Option<PendingLaunch>> {
    pending_launch_in(&pending_dir()?, key)
}

fn pending_launch_in(folder: &Path, key: &str) -> Result<Option<PendingLaunch>> {
    if !folder.exists() {
        return Ok(None);
    }
    let mut found = None;
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        let bytes = fs::read(&path)?;
        if bytes.len() > 16 * 1024 {
            bail!("pending launch record is too large");
        }
        let record: PendingLaunch = serde_json::from_slice(&bytes)?;
        if record_path(folder, &record)? != path {
            bail!("pending launch record name does not match its operation");
        }
        if record.key == key {
            if found.is_some() {
                bail!("multiple pending launches for the same terminal");
            }
            found = Some(record);
        }
    }
    Ok(found)
}

fn record_path(folder: &Path, record: &PendingLaunch) -> Result<PathBuf> {
    Uuid::parse_str(&record.operation_id)?;
    Ok(folder.join(format!("{}.json", record.operation_id)))
}

fn save_pending_launch(record: &PendingLaunch) -> Result<()> {
    save_pending_launch_in(&pending_dir()?, record)
}

fn save_pending_launch_in(folder: &Path, record: &PendingLaunch) -> Result<()> {
    fs::create_dir_all(folder)?;
    let path = record_path(folder, record)?;
    anyhow::ensure!(!path.exists(), "pending launch record already exists");
    brachio_fs::write_atomic(&path, &serde_json::to_vec(record)?)?;
    Ok(())
}

fn remove_pending_launch(record: &PendingLaunch) -> Result<()> {
    fs::remove_file(record_path(&pending_dir()?, record)?)?;
    Ok(())
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
    (client, key): (Client, String),
    first: Reply,
    mut following: Following,
    channel: Channel<InvokeResponseBody>,
    agent: bool,
    forwarding: Arc<AtomicBool>,
) {
    let mut last_status = None;
    let status = agent.then(|| TerminalStatus::new(channel.clone()));
    let mut live = false;
    let mut next = Some(first);
    loop {
        if let Some(mut reply) = next.take() {
            let forward = forwarding.load(Ordering::Relaxed);
            if live && forward && skipped_output(&reply) {
                // The runtime dropped output this pane never received. Redraw from a fresh frame
                // instead of showing a screen with a hole in it.
                send_message(&channel, &ChannelMessage::Reset { reset: true });
                match open_frame(&client, &key).await {
                    Ok((frame, resumed)) => {
                        reply = frame;
                        following = resumed;
                    }
                    Err(_) => {
                        send_message(&channel, &ChannelMessage::Detached { detached: true });
                        return;
                    }
                }
            }
            if deliver(
                reply,
                &mut last_status,
                &channel,
                status.as_ref(),
                live,
                forward,
            ) {
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

/// Whether the runtime no longer held output this follower had not yet read.
fn skipped_output(reply: &Reply) -> bool {
    matches!(
        &reply.result,
        ResponseResult::Success { output: Some(page), .. } if page.truncated
    )
}

/// Send one reply on; `true` once the program has ended and its output is fully sent.
fn deliver(
    reply: Reply,
    last_status: &mut Option<StatusInfo>,
    channel: &Channel<InvokeResponseBody>,
    status: Option<&TerminalStatus>,
    live: bool,
    forward: bool,
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
        if forward {
            send(channel, InvokeResponseBody::Raw(bytes));
        }
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

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::{
        locate, spawn, Attached, Client, ClientError, RuntimeTerminals, HOSTED_HANDLE_BASE,
    };

    /// Short enough for the Unix socket path limit on macOS, where temp directories are long.
    fn missing_runtime_name() -> String {
        format!(
            "missing-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        )
    }

    fn attach_to_dead_runtime(terminals: &RuntimeTerminals) -> SessionId {
        let id = terminals.new_handle();
        let client = Client::new(&missing_runtime_name());
        let pump = spawn(std::future::pending::<()>());
        terminals.attached.lock().expect("lock").insert(
            id,
            Attached {
                key: "test:agent".to_owned(),
                instance: "test".to_owned(),
                session_id: 1,
                client,
                pump,
                forwarding: Arc::new(AtomicBool::new(true)),
            },
        );
        id
    }

    #[tokio::test]
    async fn detach_drops_the_attachment_without_telling_the_runtime_to_stop() {
        let terminals = RuntimeTerminals::default();
        let id = attach_to_dead_runtime(&terminals);
        // A stop would call the unreachable runtime and fail; a detach must not call it at all.
        assert!(terminals.detach(id).expect("hosted").is_ok());
        assert!(terminals.attached.lock().expect("lock").is_empty());
        assert!(terminals.detach(id).is_none());
    }

    #[tokio::test]
    async fn stop_calls_the_runtime_unlike_detach() {
        let terminals = RuntimeTerminals::default();
        let id = attach_to_dead_runtime(&terminals);
        assert!(terminals.stop(id).await.expect("hosted").is_err());
        assert!(terminals.attached.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn an_attachment_is_released_when_its_pump_ends() {
        let terminals = RuntimeTerminals::default();
        let id = terminals.new_handle();
        let client = Client::new(&missing_runtime_name());
        terminals
            .register(
                id,
                "test:agent".to_owned(),
                client,
                ("test".to_owned(), 1),
                &Arc::new(AtomicBool::new(true)),
                async {},
            )
            .expect("register");
        for _ in 0..100 {
            if terminals.attached.lock().expect("lock").is_empty() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("the finished pump left its attachment behind");
    }

    #[tokio::test]
    async fn parking_stops_output_forwarding_but_keeps_the_attachment() {
        let terminals = RuntimeTerminals::default();
        let id = attach_to_dead_runtime(&terminals);
        assert!(terminals.park(id).expect("hosted").is_ok());
        let attached = terminals.attached.lock().expect("lock");
        let found = attached.get(&id).expect("still attached");
        assert!(!found.forwarding.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn attaching_a_key_again_replaces_its_parked_attachment_only() {
        let terminals = RuntimeTerminals::default();
        let parked = attach_to_dead_runtime(&terminals);
        let other = attach_to_dead_runtime(&terminals);
        terminals.park(parked).expect("hosted").expect("parked");
        let client = Client::new(&missing_runtime_name());
        let fresh = terminals.new_handle();
        terminals
            .register(
                fresh,
                "test:agent".to_owned(),
                client,
                ("test".to_owned(), 1),
                &Arc::new(AtomicBool::new(true)),
                std::future::pending::<()>(),
            )
            .expect("register");
        let attached = terminals.attached.lock().expect("lock");
        assert!(!attached.contains_key(&parked));
        assert!(attached.contains_key(&other) && attached.contains_key(&fresh));
    }

    #[test]
    fn parking_an_unknown_handle_is_not_hosted() {
        let terminals = RuntimeTerminals::default();
        assert!(terminals
            .park(SessionId::from_raw(HOSTED_HANDLE_BASE + 99))
            .is_none());
    }

    #[test]
    fn detaching_an_unknown_handle_is_not_hosted() {
        let terminals = RuntimeTerminals::default();
        assert!(terminals
            .detach(SessionId::from_raw(HOSTED_HANDLE_BASE + 99))
            .is_none());
    }

    #[tokio::test]
    async fn a_missing_current_runtime_is_distinct_from_a_missing_terminal() {
        let client = Client::new(&missing_runtime_name());
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
    use std::path::PathBuf;

    use brachio_runtime::wire::{
        LostInfo, OutputPage, Reply, ResponseResult, ScreenSnapshot, SessionInfo,
    };

    use super::{saved_screen_from, skipped_output, task_terminal_keys, SavedScreen};

    fn page_reply(truncated: bool) -> Reply {
        Reply {
            version: 1,
            instance: "i".into(),
            result: ResponseResult::Success {
                session: SessionInfo {
                    session_id: serde_json::from_str("1").expect("session id"),
                    pid: 7,
                    output_bytes: 300_000,
                    exited: false,
                    exit_code: None,
                    status: None,
                },
                output: Some(OutputPage {
                    start: 40_000,
                    next: 48_000,
                    end: 300_000,
                    truncated,
                    bytes: Vec::new(),
                }),
            },
        }
    }

    #[test]
    fn a_page_that_starts_after_the_requested_offset_means_output_was_skipped() {
        assert!(skipped_output(&page_reply(true)));
        assert!(!skipped_output(&page_reply(false)));
    }

    #[test]
    fn a_recovered_screen_keeps_its_frame_and_how_the_program_ended() {
        let result = ResponseResult::Recovered {
            lost: LostInfo {
                key: "t1:agent".into(),
                cwd: PathBuf::from("w"),
                former_instance: "i".into(),
                former_pid: 7,
                exited: true,
                exit_code: Some(2),
                output_bytes: 9,
                saved_at_ms: 1,
            },
            screen: ScreenSnapshot {
                ansi: "done".into(),
                cursor: (0, 4),
                rows: 24,
                cols: 80,
                alternate_screen: false,
                resume_offset: None,
            },
        };
        assert_eq!(
            saved_screen_from(result),
            Some(SavedScreen {
                ansi: "done".into(),
                rows: 24,
                cols: 80,
                exited: true,
                exit_code: Some(2),
            })
        );
        assert_eq!(
            saved_screen_from(ResponseResult::Lost { lost: vec![] }),
            None
        );
    }

    #[test]
    fn a_task_names_its_fixed_terminals_and_every_extra_tab() {
        assert_eq!(
            task_terminal_keys("t1", &["abc".to_owned(), "old".to_owned()]),
            [
                "t1:agent",
                "t1:shell",
                "t1:agent-abc",
                "t1:shell-abc",
                "t1:agent-old",
                "t1:shell-old"
            ]
        );
        assert_eq!(task_terminal_keys("t1", &[]), ["t1:agent", "t1:shell"]);
    }
}

#[cfg(test)]
mod pending_launch_tests {
    use super::*;

    fn launch_with_argument(arg: String) -> Action {
        Action::LaunchOnce {
            instance: "instance".into(),
            operation_id: "operation".into(),
            key: "key".into(),
            cwd: PathBuf::from("/work"),
            program: "claude".into(),
            args: vec![arg],
            env: Vec::new(),
            rows: 24,
            cols: 80,
        }
    }

    #[test]
    fn a_launch_over_the_runtime_request_limit_is_refused_before_it_is_recorded() {
        assert!(ensure_request_fits(&launch_with_argument("x".repeat(70 * 1024))).is_err());
        assert!(ensure_request_fits(&launch_with_argument("x".repeat(1024))).is_ok());
    }

    #[test]
    fn typed_input_is_split_without_breaking_characters() {
        let text = "한".repeat(20_000);
        let chunks: Vec<&str> = input_chunks(&text).collect();
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.len() <= WRITE_CHUNK_BYTES));
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn pending_record_survives_reopen_and_contains_no_command_data() {
        let folder = tempfile::tempdir().expect("tempdir");
        let pending = PendingLaunch {
            key: "task:agent".into(),
            instance: Uuid::new_v4().to_string(),
            operation_id: Uuid::new_v4().to_string(),
        };
        save_pending_launch_in(folder.path(), &pending).expect("save");
        let loaded = pending_launch_in(folder.path(), "task:agent")
            .expect("read")
            .expect("pending");
        assert_eq!(loaded.instance, pending.instance);
        assert_eq!(loaded.operation_id, pending.operation_id);
        assert!(pending_launch_in(folder.path(), "other")
            .expect("read")
            .is_none());
        let contents = fs::read_to_string(record_path(folder.path(), &pending).expect("path"))
            .expect("record");
        assert!(!contents.contains("program"));
        assert!(!contents.contains("env"));
        assert!(save_pending_launch_in(folder.path(), &pending).is_err());
    }

    fn record() -> PendingLaunch {
        PendingLaunch {
            key: "task:agent".into(),
            instance: Uuid::new_v4().to_string(),
            operation_id: Uuid::new_v4().to_string(),
        }
    }

    fn success() -> ResponseResult {
        ResponseResult::LaunchPending {
            launch_pending: true,
        }
    }

    #[test]
    fn a_launch_the_runtime_finished_is_reattached() {
        let outcome = ResponseResult::LaunchCompleted {
            launch_outcome: Box::new(success()),
        };
        assert!(matches!(
            reconcile_launch(&record(), Ok(outcome)),
            Ok(Reconciled::Attach(_))
        ));
    }

    #[test]
    fn a_launch_that_failed_is_reported_as_not_started() {
        let outcome = ResponseResult::LaunchCompleted {
            launch_outcome: Box::new(ResponseResult::Error {
                error: "spawn failed".into(),
            }),
        };
        let Ok(Reconciled::NotStarted(error)) = reconcile_launch(&record(), Ok(outcome)) else {
            panic!("expected not started");
        };
        assert_eq!(error, "spawn failed");
    }

    #[test]
    fn a_launch_the_runtime_never_saw_or_a_new_runtime_is_uncertain() {
        let pending = record();
        for message in [
            "launch operation was not observed by this runtime",
            "runtime instance changed; launch outcome is unknown",
        ] {
            let error = reconcile_launch(&pending, Err(ClientError::Refused(message.into())))
                .err()
                .expect("uncertain");
            assert_eq!(
                error.to_string(),
                format!("BRACHIO_LAUNCH_UNCERTAIN:{}", pending.operation_id)
            );
        }
    }

    #[test]
    fn an_unreachable_runtime_or_running_launch_never_forgets_the_record() {
        let pending = record();
        let error = reconcile_launch(&pending, Err(ClientError::NotRunning))
            .err()
            .expect("error");
        assert!(!error.to_string().contains("BRACHIO_LAUNCH_UNCERTAIN"));
        assert!(reconcile_launch(&pending, Ok(success())).is_err());
    }

    #[test]
    fn corrupted_pending_records_fail_closed() {
        let folder = tempfile::tempdir().expect("tempdir");
        fs::write(folder.path().join("broken.json"), b"{").expect("write");
        assert!(pending_launch_in(folder.path(), "task:agent").is_err());
    }
}

#[cfg(test)]
#[path = "runtime_terminal_flow_tests.rs"]
mod flow_tests;
