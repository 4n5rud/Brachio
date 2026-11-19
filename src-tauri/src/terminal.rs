//! IPC commands for terminals: start a shell or an agent in a PTY, send
//! input, resize, and stop. Output goes back over a per-session channel as
//! raw bytes, so the web view gets an `ArrayBuffer` with no JSON encoding.

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use brachio_agent::{AgentKind, Installed, PromptDelivery, ResolvedCommand};
use brachio_core::status::AgentState;
use brachio_pty::{EventSink, PtyEvent, SessionId, SessionManager, SpawnSpec};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::hook_setup::HookSetup;
use crate::runtime_terminals::{Launch, RuntimeTerminals};
use crate::{task_ports, workspace::Database};

/// App state that owns every terminal.
pub struct Terminals(pub Arc<SessionManager>);

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
    /// Whether the agent continued an earlier conversation.
    pub resumed: bool,
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
pub fn agents_detect() -> Vec<Installed> {
    brachio_agent::detect_all()
}

/// Opens a terminal and streams its events to `on_event`.
#[tauri::command]
pub async fn terminal_open(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    database: State<'_, Database>,
    hook: State<'_, HookSetup>,
    request: OpenRequest,
    on_event: Channel<InvokeResponseBody>,
) -> CommandResult<Opened> {
    let (mut command, prompt_delivery, resumed) = match request.agent {
        Some(kind) => agent_command(kind, &request)?,
        None => (shell_command(), PromptDelivery::None, false),
    };
    // Claude Code can ask the app instead of the terminal when a hook is available.
    let mut environment = terminal_env();
    if let Some(key) = request.key.as_deref() {
        match task_ports::port_for_terminal(&database, key, &request.cwd) {
            Ok(Some(port)) => environment.push(("BRACHIO_PORT".to_owned(), port.to_string())),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "could not assign a development port"),
        }
    }
    if let (Some(AgentKind::ClaudeCode), Some(key)) = (request.agent, request.key.as_deref()) {
        let extra = hook.environment(key);
        if !extra.is_empty() {
            command.args.splice(0..0, hook.claude_arguments());
            environment.extend(extra);
        }
    }
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
        let opened = hosted.open(key, launch, on_event.clone()).await;
        if let Some((id, attached)) = opened.map_err(|error| to_message(&error))? {
            return Ok(Opened {
                id,
                prompt_delivery: if attached {
                    PromptDelivery::None
                } else {
                    prompt_delivery
                },
                resumed: resumed || attached,
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
    let spawned = terminals.0.spawn(&spec, sink_for(on_event));
    let id = spawned.map_err(|error| to_message(&error))?;
    Ok(Opened {
        id,
        prompt_delivery,
        resumed,
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

/// Stops a terminal's process. The exit event still arrives on its channel.
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

/// The command for an agent: its earlier conversation when asked to resume
/// and one exists, otherwise a new one.
fn agent_command(
    kind: AgentKind,
    request: &OpenRequest,
) -> CommandResult<(ResolvedCommand, PromptDelivery, bool)> {
    let installed = installed(kind)?;
    let earlier = if request.resume {
        earlier_conversation(kind, request.session_id.as_deref(), &request.cwd)
    } else {
        None
    };
    if let Some(conversation) = earlier {
        let command = installed.resolve(&kind.resume(&conversation));
        return Ok((command, PromptDelivery::None, true));
    }
    let session_id = request
        .session_id
        .as_deref()
        .filter(|_| kind.accepts_session_id());
    let (command, delivery) = installed.resolve_interactive(request.prompt.as_deref(), session_id);
    Ok((command, delivery, false))
}

/// The conversation a task left behind. Claude Code's id was pinned when the
/// task was created; Codex's is found from its own records for the worktree.
fn earlier_conversation(kind: AgentKind, session_id: Option<&str>, cwd: &Path) -> Option<String> {
    match kind {
        AgentKind::ClaudeCode => session_id.map(str::to_owned),
        AgentKind::Codex => brachio_agent::latest_codex_session(cwd),
    }
}

fn installed(kind: AgentKind) -> CommandResult<Installed> {
    brachio_agent::detect(kind)
        .ok_or_else(|| format!("{} is not installed or not on PATH", kind.program()))
}

fn shell_command() -> ResolvedCommand {
    ResolvedCommand {
        program: default_shell(),
        args: Vec::new(),
    }
}

fn sink_for(channel: Channel<InvokeResponseBody>) -> EventSink {
    Arc::new(move |event| {
        let message = match event {
            PtyEvent::Output(bytes) => {
                send(&channel, InvokeResponseBody::Raw(bytes));
                return;
            }
            PtyEvent::Exit(exit_code) => ChannelMessage::Exit { exit_code },
            PtyEvent::Status(report) => ChannelMessage::Status {
                status: report.state,
                message: report.message,
            },
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
