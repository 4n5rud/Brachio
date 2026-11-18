//! IPC commands for terminals: start a shell or an agent in a PTY, send
//! input, resize, and stop. Output goes back over a per-session channel as
//! raw bytes, so the web view gets an `ArrayBuffer` with no JSON encoding.

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use brachio_agent::{AgentKind, Installed, PromptDelivery, ResolvedCommand};
use brachio_core::status::AgentState;
use brachio_pty::{EventSink, PtyEvent, SessionId, SessionManager, SpawnSpec};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

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
    /// Continue the agent's latest conversation instead of starting fresh.
    #[serde(default)]
    pub resume: bool,
    /// Working directory.
    pub cwd: PathBuf,
    /// Initial height in rows.
    pub rows: u16,
    /// Initial width in columns.
    pub cols: u16,
}

/// A terminal that started.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// Session handle for later calls.
    pub id: SessionId,
    /// Whether the UI still has to type the first prompt in.
    pub prompt_delivery: PromptDelivery,
}

/// Sent as JSON on the channel. Output frames are raw bytes instead.
#[derive(Debug, Serialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
enum ChannelMessage {
    /// The child exited.
    Exit { exit_code: Option<u32> },
    /// The agent reported its state.
    Status {
        status: AgentState,
        message: Option<String>,
    },
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
pub fn terminal_open(
    terminals: State<'_, Terminals>,
    request: OpenRequest,
    on_event: Channel<InvokeResponseBody>,
) -> CommandResult<Opened> {
    let (command, prompt_delivery) = match request.agent {
        Some(kind) if request.resume => (
            installed(kind)?.resolve(&kind.resume()),
            PromptDelivery::None,
        ),
        Some(kind) => installed(kind)?.resolve_interactive(request.prompt.as_deref()),
        None => (shell_command(), PromptDelivery::None),
    };
    let spec = SpawnSpec {
        program: command.program.to_string_lossy().into_owned(),
        args: command.args,
        cwd: request.cwd,
        env: terminal_env(),
        rows: request.rows.max(1),
        cols: request.cols.max(1),
    };
    let spawned = terminals.0.spawn(&spec, sink_for(on_event));
    let id = spawned.map_err(|error| to_message(&error))?;
    Ok(Opened {
        id,
        prompt_delivery,
    })
}

/// Sends keyboard input to a terminal.
#[tauri::command]
pub fn terminal_write(
    terminals: State<'_, Terminals>,
    id: SessionId,
    data: String,
) -> CommandResult<()> {
    terminals
        .0
        .write(id, data.as_bytes())
        .map_err(|error| to_message(&error))
}

/// Changes a terminal's size.
#[tauri::command]
pub fn terminal_resize(
    terminals: State<'_, Terminals>,
    id: SessionId,
    rows: u16,
    cols: u16,
) -> CommandResult<()> {
    let resized = terminals.0.resize(id, rows.max(1), cols.max(1));
    resized.map_err(|error| to_message(&error))
}

/// Stops a terminal's process. The exit event still arrives on its channel.
#[tauri::command]
pub fn terminal_close(terminals: State<'_, Terminals>, id: SessionId) -> CommandResult<()> {
    terminals.0.kill(id).map_err(|error| to_message(&error))
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

fn send(channel: &Channel<InvokeResponseBody>, body: InvokeResponseBody) {
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
