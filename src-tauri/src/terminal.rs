//! IPC commands for terminals: start a shell or an agent in a PTY, send
//! input, resize, and stop. Output goes back over a per-session channel as
//! raw bytes, so the web view gets an `ArrayBuffer` with no JSON encoding.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use brachio_agent::{AgentKind, Installed, PromptDelivery, ResolvedCommand};
use brachio_core::status::AgentState;
use brachio_pty::{EventSink, PtyEvent, SessionId, SessionManager, SpawnSpec};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::hook_setup::HookSetup;
use crate::runtime_terminals::{Launch, RuntimeTerminals};
use crate::terminal_status::TerminalStatus;
use crate::{task_ports, terminal_authorization, workspace::Database};

/// App state that owns every terminal.
pub struct Terminals(pub Arc<SessionManager>, pub SessionKeys);

/// Which task terminal key each in-app session was opened for; the manager only knows handles.
#[derive(Default)]
pub struct SessionKeys(Mutex<HashMap<SessionId, String>>);

impl SessionKeys {
    pub(crate) fn record(&self, id: SessionId, key: String) {
        if let Ok(mut keys) = self.0.lock() {
            keys.insert(id, key);
        }
    }

    /// Keys of the task's in-app terminals whose program is still running.
    ///
    /// # Errors
    /// Fails when the session table cannot be read, so the caller can refuse rather than guess.
    pub fn live_for_task(
        &self,
        manager: &SessionManager,
        task_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        let live = manager.ids()?;
        let mut keys = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal key list lock poisoned"))?;
        keys.retain(|id, _| live.contains(id));
        let prefix = format!("{task_id}:");
        let mut found: Vec<String> = keys
            .values()
            .filter(|key| key.starts_with(&prefix))
            .cloned()
            .collect();
        found.sort();
        Ok(found)
    }
}

/// What to run in a new terminal.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRequest {
    /// `None` opens the user's shell.
    pub agent: Option<AgentKind>,
    /// First prompt for the agent. Ignored for a shell.
    pub prompt: Option<String>,
    /// Continue the task's conversation instead of starting a new one.
    #[serde(default)]
    pub resume: bool,
    /// The task's conversation id, for agents that take one.
    pub session_id: Option<String>,
    /// A Codex conversation picked explicitly from the worktree's history.
    pub codex_conversation_id: Option<String>,
    /// Working directory.
    pub cwd: PathBuf,
    /// Initial height in rows.
    pub rows: u16,
    /// Initial width in columns.
    pub cols: u16,
    /// Stable name of this terminal, such as the task and tab. With it the terminal can live in
    /// the runtime and be found again after the window closes.
    #[serde(default)]
    pub key: Option<String>,
}

/// A terminal that started.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// Session handle for later calls.
    pub id: SessionId,
    /// Whether the UI still has to type the first prompt in.
    pub prompt_delivery: PromptDelivery,
    /// Whether setup input and first prompts should be suppressed.
    pub resumed: bool,
    /// Whether the runtime already owned this live process.
    pub attached: bool,
}

/// Sent as JSON on the channel. Output frames are raw bytes instead.
#[derive(Debug, Serialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub(crate) enum ChannelMessage {
    /// The child exited.
    Exit { exit_code: Option<u32> },
    /// The agent reported its state.
    Status {
        status: AgentState,
        message: Option<String>,
    },
    /// The connection to the runtime ended while the program may still run.
    Detached { detached: bool },
    /// The screen just sent is the last one of a shell that is gone, shown before its restart.
    Restored { restored: bool },
}

/// Errors cross the IPC boundary as display strings.
type CommandResult<T> = Result<T, String>;

fn to_message(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

/// Lists the agent CLIs found on `PATH`.
#[tauri::command]
pub async fn agents_detect() -> Vec<Installed> {
    tokio::task::spawn_blocking(brachio_agent::detect_all)
        .await
        .unwrap_or_default()
}

/// Returns Codex resume choices for a task tab that has no live runtime session.
#[tauri::command]
pub async fn terminal_codex_conversations(
    database: State<'_, Database>,
    hosted: State<'_, RuntimeTerminals>,
    key: String,
) -> CommandResult<Vec<brachio_agent::CodexConversation>> {
    let owner = terminal_authorization::resolve(&*database.lock()?, &key)?;
    if owner.agent != Some(AgentKind::Codex) {
        return Err("This terminal is not a Codex tab.".to_owned());
    }
    if hosted
        .has_session(&key)
        .await
        .map_err(|error| to_message(&error))?
    {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(move || {
        brachio_agent::codex_conversations(&owner.task.worktree_path)
            .into_iter()
            .filter(|candidate| uuid::Uuid::parse_str(&candidate.id).is_ok())
            .collect()
    })
    .await
    .map_err(|error| error.to_string())
}

/// Clear a specific uncertain launch after the user has confirmed the risk.
#[tauri::command]
pub async fn terminal_forget_uncertain_launch(
    hosted: State<'_, RuntimeTerminals>,
    database: State<'_, Database>,
    key: String,
    operation_id: String,
) -> CommandResult<()> {
    terminal_authorization::resolve(&*database.lock()?, &key)?;
    hosted
        .forget_uncertain_launch(&key, &operation_id)
        .await
        .map_err(|error| error.to_string())
}

/// Opens a terminal and streams its events to `on_event`.
#[tauri::command]
pub async fn terminal_open(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    database: State<'_, Database>,
    hook: State<'_, HookSetup>,
    mut request: OpenRequest,
    on_event: Channel<InvokeResponseBody>,
) -> CommandResult<Opened> {
    let key = request
        .key
        .as_deref()
        .ok_or_else(|| "A registered task terminal is required.".to_owned())?;
    let owner = terminal_authorization::resolve(&*database.lock()?, key)?;
    let worktree = owner
        .task
        .worktree_path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let requested = request
        .cwd
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if requested != worktree || request.agent != owner.agent {
        return Err("Terminal request does not match the registered task tab.".to_owned());
    }
    request.cwd = owner.task.worktree_path;
    request.session_id = owner.conversation;
    if request.codex_conversation_id.is_some()
        && (!request.resume || request.agent != Some(AgentKind::Codex))
    {
        return Err("A Codex conversation can only be resumed in a Codex tab.".to_owned());
    }
    // Claude Code can ask the app instead of the terminal when a hook is available.
    let mut environment = terminal_env();
    if let Some(key) = request.key.as_deref() {
        match task_ports::port_for_terminal(&database, key, &request.cwd) {
            Ok(Some(port)) => environment.push(("BRACHIO_PORT".to_owned(), port.to_string())),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "could not assign a development port"),
        }
    }
    let mut options = Vec::new();
    if let (Some(AgentKind::ClaudeCode), Some(key)) = (request.agent, request.key.as_deref()) {
        if !hook.environment(key).is_empty() {
            options = hook.claude_arguments();
        }
    }
    // Every agent, not only Claude Code, can send and read messages.
    if let (Some(_), Some(key)) = (request.agent, request.key.as_deref()) {
        environment.extend(hook.mail_environment(key));
    }
    let (command, prompt_delivery, resumed) = match request.agent {
        Some(kind) => {
            let (prepared, returned_request) = tauri::async_runtime::spawn_blocking(move || {
                let result = agent_command(kind, &request, options);
                (result, request)
            })
            .await
            .map_err(|error| error.to_string())?;
            request = returned_request;
            prepared?
        }
        None => (shell_command(), PromptDelivery::None, false),
    };
    environment.extend(command.env);
    if let Some(key) = request.key.clone() {
        let launch = Launch {
            cwd: request.cwd.clone(),
            program: command.program.to_string_lossy().into_owned(),
            args: command.args.clone(),
            env: environment
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect(),
            rows: request.rows.max(1),
            cols: request.cols.max(1),
        };
        let opened = hosted
            .open(key, launch, on_event.clone(), request.agent.is_some())
            .await;
        if let Some((id, attached)) = opened.map_err(|error| to_message(&error))? {
            return Ok(Opened {
                id,
                prompt_delivery: if attached {
                    PromptDelivery::None
                } else {
                    prompt_delivery
                },
                resumed: resumed || attached,
                attached,
            });
        }
    }
    let spec = SpawnSpec {
        program: command.program.to_string_lossy().into_owned(),
        args: command.args,
        cwd: request.cwd,
        env: environment,
        rows: request.rows.max(1),
        cols: request.cols.max(1),
    };
    let manager = Arc::clone(&terminals.0);
    let sink = sink_for(on_event, request.agent.is_some());
    let spawned = tauri::async_runtime::spawn_blocking(move || manager.spawn(&spec, sink))
        .await
        .map_err(|error| error.to_string())?;
    let id = spawned.map_err(|error| to_message(&error))?;
    if let Some(key) = request.key.clone() {
        terminals.1.record(id, key);
    }
    Ok(Opened {
        id,
        prompt_delivery,
        resumed,
        attached: false,
    })
}

/// Sends keyboard input to a terminal.
#[tauri::command]
pub async fn terminal_write(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    id: SessionId,
    data: String,
) -> CommandResult<()> {
    if let Some(result) = hosted.write(id, data.clone()).await {
        return result.map_err(|error| to_message(&error));
    }
    terminals
        .0
        .write(id, data.as_bytes())
        .map_err(|error| to_message(&error))
}

/// Changes a terminal's size.
#[tauri::command]
pub async fn terminal_resize(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    id: SessionId,
    rows: u16,
    cols: u16,
) -> CommandResult<()> {
    let (rows, cols) = (rows.max(1), cols.max(1));
    if let Some(result) = hosted.resize(id, rows, cols).await {
        return result.map_err(|error| to_message(&error));
    }
    let resized = terminals.0.resize(id, rows, cols);
    resized.map_err(|error| to_message(&error))
}

/// Stops a terminal's program for good. The exit event still arrives on its channel.
/// Use `terminal_detach` to only stop watching it.
#[tauri::command]
pub async fn terminal_close(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    id: SessionId,
) -> CommandResult<()> {
    if let Some(result) = hosted.stop(id).await {
        return result.map_err(|error| to_message(&error));
    }
    terminals.0.kill(id).map_err(|error| to_message(&error))
}

/// Stops the UI from watching a runtime-hosted terminal; its program keeps running and opening the
/// same key reattaches. An in-app terminal has no runtime to keep it, so it is refused rather than
/// killed or orphaned.
#[tauri::command]
pub fn terminal_detach(hosted: State<'_, RuntimeTerminals>, id: SessionId) -> CommandResult<()> {
    match hosted.detach(id) {
        Some(result) => result.map_err(|error| to_message(&error)),
        None => Err("this terminal is not hosted by the runtime and cannot be detached".to_owned()),
    }
}

/// The command for an agent: its earlier conversation when asked to resume
/// and one exists, otherwise a new one.
fn agent_command(
    kind: AgentKind,
    request: &OpenRequest,
    mut options: Vec<String>,
) -> CommandResult<(ResolvedCommand, PromptDelivery, bool)> {
    let installed = installed(kind)?;
    let earlier = if let Some(chosen) = &request.codex_conversation_id {
        if uuid::Uuid::parse_str(chosen).is_err()
            || !brachio_agent::codex_conversations(&request.cwd)
                .iter()
                .any(|candidate| candidate.id == *chosen)
        {
            return Err(
                "The chosen Codex conversation is no longer available for this worktree."
                    .to_owned(),
            );
        }
        Some(chosen.clone())
    } else if request.resume {
        earlier_conversation(kind, request.session_id.as_deref())
    } else {
        None
    };
    if let Some(conversation) = earlier {
        let mut launch = kind.resume(&conversation);
        options.append(&mut launch.args);
        launch.args = options;
        let command = installed
            .resolve(&launch)
            .map_err(|error| error.to_string())?;
        return Ok((command, PromptDelivery::None, true));
    }
    let session_id = request
        .session_id
        .as_deref()
        .filter(|_| kind.accepts_session_id());
    let (command, delivery) = installed
        .resolve_interactive(request.prompt.as_deref(), session_id, options)
        .map_err(|error| error.to_string())?;
    Ok((command, delivery, false))
}

/// Resume only an identity known to belong to this tab.
fn earlier_conversation(kind: AgentKind, session_id: Option<&str>) -> Option<String> {
    match kind {
        AgentKind::ClaudeCode => session_id.map(str::to_owned),
        // The Codex tab UUID identifies the PTY, not a Codex conversation.
        AgentKind::Codex => None,
    }
}

fn installed(kind: AgentKind) -> CommandResult<Installed> {
    brachio_agent::locate(kind)
        .ok_or_else(|| format!("{} is not installed or not on PATH", kind.program()))
}

fn shell_command() -> ResolvedCommand {
    ResolvedCommand {
        program: default_shell(),
        args: Vec::new(),
        env: Vec::new(),
    }
}

fn sink_for(channel: Channel<InvokeResponseBody>, agent: bool) -> EventSink {
    let status = agent.then(|| TerminalStatus::new(channel.clone()));
    Arc::new(move |event| {
        let message = match event {
            PtyEvent::Output(bytes) => {
                if let Some(status) = &status {
                    status.activity();
                }
                send(&channel, InvokeResponseBody::Raw(bytes));
                return;
            }
            PtyEvent::Exit(exit_code) => ChannelMessage::Exit { exit_code },
            PtyEvent::Status(report) => {
                if let Some(status) = &status {
                    status.osc(report.state, report.message);
                }
                return;
            }
        };
        if let Ok(json) = serde_json::to_string(&message) {
            send(&channel, InvokeResponseBody::Json(json));
        }
    })
}

pub(crate) fn send_message(channel: &Channel<InvokeResponseBody>, message: &ChannelMessage) {
    if let Ok(json) = serde_json::to_string(message) {
        send(channel, InvokeResponseBody::Json(json));
    }
}

pub(crate) fn send(channel: &Channel<InvokeResponseBody>, body: InvokeResponseBody) {
    if let Err(error) = channel.send(body) {
        tracing::debug!("terminal channel closed: {error}");
    }
}

fn default_shell() -> PathBuf {
    let (variable, fallback) = if cfg!(windows) {
        ("COMSPEC", "cmd.exe")
    } else if cfg!(target_os = "macos") {
        ("SHELL", "/bin/zsh")
    } else {
        ("SHELL", "/bin/sh")
    };
    env::var_os(variable).map_or_else(|| PathBuf::from(fallback), PathBuf::from)
}

fn terminal_env() -> Vec<(String, String)> {
    vec![
        ("TERM".to_owned(), "xterm-256color".to_owned()),
        ("COLORTERM".to_owned(), "truecolor".to_owned()),
        ("TERM_PROGRAM".to_owned(), "Brachio".to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_messages_use_camel_case_json() {
        let exit = serde_json::to_string(&ChannelMessage::Exit { exit_code: Some(3) })
            .expect("serialize exit");
        assert_eq!(exit, r#"{"exitCode":3}"#);

        let status = serde_json::to_string(&ChannelMessage::Status {
            status: AgentState::Working,
            message: None,
        })
        .expect("serialize status");
        assert_eq!(status, r#"{"status":{"state":"working"},"message":null}"#);
    }
}

#[cfg(test)]
mod session_key_tests {
    use std::sync::Arc;

    use brachio_pty::{EventSink, SessionId, SessionManager, SpawnSpec};

    use super::SessionKeys;

    fn long_running() -> SpawnSpec {
        let (program, args) = if cfg!(windows) {
            ("cmd.exe", vec!["/c", "ping", "-n", "60", "127.0.0.1"])
        } else {
            ("sleep", vec!["60"])
        };
        SpawnSpec {
            program: program.to_owned(),
            args: args.into_iter().map(str::to_owned).collect(),
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        }
    }

    #[test]
    fn a_running_in_app_session_is_found_by_its_task_and_a_gone_one_is_not() {
        let manager = Arc::new(SessionManager::default());
        let sink: EventSink = Arc::new(|_| {});
        let live = manager.spawn(&long_running(), sink).expect("spawn");
        let keys = SessionKeys::default();
        keys.record(live, "t1:agent".to_owned());
        keys.record(SessionId::from_raw(999), "t1:shell".to_owned());
        keys.record(live, "t1:agent".to_owned());

        assert_eq!(
            keys.live_for_task(&manager, "t1").expect("live"),
            ["t1:agent"]
        );
        assert!(keys.live_for_task(&manager, "t2").expect("live").is_empty());
        manager.kill(live).expect("kill");
    }
}
