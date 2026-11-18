//! Native CLI authentication and read-only Codex quota reporting.

use std::env;
use std::process::Stdio;
use std::time::Duration;

use brachio_agent::{find_on_path, AgentKind, ResolvedCommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
const QUOTA_TIMEOUT: Duration = Duration::from_secs(15);
const PROTOCOL_LIMIT: u64 = 512 * 1024;

fn cli(kind: AgentKind, args: &[&str]) -> Result<Command, String> {
    let path = env::var_os("PATH");
    let extensions = env::var_os("PATHEXT");
    let executable = find_on_path(kind.program(), path.as_deref(), extensions.as_deref())
        .ok_or_else(|| format!("{} is not installed or not on PATH", kind.program()))?;
    let args: Vec<_> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let resolved = ResolvedCommand::new(&executable, &args);
    let mut command = Command::new(resolved.program);
    command.args(resolved.args).kill_on_drop(true);
    Ok(command)
}

/// Only account metadata intended for display is returned.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAuthStatus {
    signed_in: bool,
    account: Option<String>,
}

/// Reads sign-in status through the installed CLI.
#[tauri::command]
pub async fn agent_auth_status(kind: AgentKind) -> Result<AgentAuthStatus, String> {
    let args: &[&str] = match kind {
        AgentKind::Codex => &["login", "status"],
        AgentKind::ClaudeCode => &["auth", "status", "--json"],
    };
    let output = timeout(
        STATUS_TIMEOUT,
        cli(kind, args)?
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| "Agent sign-in status timed out.".to_owned())?
    .map_err(|_| "Could not read agent sign-in status.".to_owned())?;
    match kind {
        AgentKind::Codex if output.status.code() == Some(0) || output.status.code() == Some(1) => {
            Ok(AgentAuthStatus {
                signed_in: output.status.success(),
                account: None,
            })
        }
        AgentKind::Codex => Err("Codex did not report its sign-in status.".to_owned()),
        AgentKind::ClaudeCode => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ClaudeStatus {
                logged_in: bool,
                email: Option<String>,
            }
            let info: ClaudeStatus = serde_json::from_slice(&output.stdout)
                .map_err(|_| "Claude Code did not report its sign-in status.".to_owned())?;
            Ok(AgentAuthStatus {
                signed_in: info.logged_in,
                account: info.email.filter(|_| info.logged_in),
            })
        }
    }
}

/// A native auth action selected by the user.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexAuthAction {
    /// Launch the CLI's sign-in flow.
    SignIn,
    /// Sign out using the CLI.
    SignOut,
}

/// Runs the explicitly selected vendor CLI command. Credentials stay with the CLI.
#[tauri::command]
pub async fn codex_auth_action(action: CodexAuthAction) -> Result<(), String> {
    let signing_in = matches!(action, CodexAuthAction::SignIn);
    let operation = if signing_in { "login" } else { "logout" };
    let mut command = cli(AgentKind::Codex, &[operation])?;
    #[cfg(windows)]
    if signing_in {
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        command.creation_flags(CREATE_NEW_CONSOLE);
    }
    // The CLI manages the browser sign-in. A Windows console lets the user also
    // see the CLI's fallback instructions during sign-in.
    if !cfg!(windows) || !signing_in {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    let duration = if signing_in {
        LOGIN_TIMEOUT
    } else {
        STATUS_TIMEOUT
    };
    let status = timeout(duration, command.status())
        .await
        .map_err(|_| "Agent authentication timed out. Retry from the agent CLI.".to_owned())?
        .map_err(|_| "Could not start agent authentication.".to_owned())?;
    if status.success() {
        Ok(())
    } else {
        Err("Agent authentication did not complete. Check the agent CLI.".to_owned())
    }
}

/// One quota window reported by the Codex app server.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    used_percent: f64,
    window_duration_mins: Option<i64>,
    resets_at: Option<i64>,
}

/// Usage windows for the active Codex account.
#[derive(Serialize, Deserialize)]
pub struct CodexQuota {
    primary: Option<QuotaWindow>,
    secondary: Option<QuotaWindow>,
}

/// Reads usage through the local Codex app-server protocol, without a model request.
#[tauri::command]
pub async fn codex_usage() -> Result<CodexQuota, String> {
    timeout(QUOTA_TIMEOUT, read_codex_usage())
        .await
        .map_err(|_| "Codex usage lookup timed out.".to_owned())?
}

async fn read_codex_usage() -> Result<CodexQuota, String> {
    let unavailable = || "Codex did not return usage for this login.".to_owned();
    let mut child = cli(AgentKind::Codex, &["app-server"])?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| unavailable())?;
    let mut input = child.stdin.take().ok_or_else(unavailable)?;
    let output = child.stdout.take().ok_or_else(unavailable)?;
    let mut lines = BufReader::new(output.take(PROTOCOL_LIMIT)).lines();
    send(
        &mut input,
        &json!({
            "id": 1,
            "method": "initialize",
            "params": { "clientInfo": { "name": "brachio", "version": env!("CARGO_PKG_VERSION") } },
        }),
    )
    .await?;
    receive(&mut lines, 1).await?;
    send(&mut input, &json!({ "method": "initialized" })).await?;
    send(
        &mut input,
        &json!({ "id": 2, "method": "account/rateLimits/read", "params": null }),
    )
    .await?;
    let mut usage = receive(&mut lines, 2).await?;
    // Closing stdio first lets npm's Windows launcher and its child exit together.
    drop(input);
    drop(lines);
    let _ = timeout(Duration::from_millis(500), child.wait()).await;
    serde_json::from_value(usage["rateLimits"].take()).map_err(|_| unavailable())
}

async fn send(input: &mut tokio::process::ChildStdin, message: &Value) -> Result<(), String> {
    let mut line = message.to_string();
    line.push('\n');
    input
        .write_all(line.as_bytes())
        .await
        .map_err(|_| "Codex usage request could not be sent.".to_owned())
}

async fn receive(
    lines: &mut tokio::io::Lines<impl tokio::io::AsyncBufRead + Unpin>,
    id: u64,
) -> Result<Value, String> {
    let unavailable = || "Codex did not return usage for this login.".to_owned();
    while let Some(line) = lines.next_line().await.map_err(|_| unavailable())? {
        let Ok(mut message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message["id"].as_u64() == Some(id) {
            return message
                .as_object_mut()
                .and_then(|message| message.remove("result"))
                .ok_or_else(unavailable);
        }
    }
    Err(unavailable())
}

/// Opens the provider's Claude usage page without forwarding any credentials.
#[tauri::command]
pub async fn open_claude_usage() -> Result<(), String> {
    const URL: &str = "https://claude.ai/settings/usage";
    #[cfg(windows)]
    let mut command = {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = Command::new("cmd.exe");
        command
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/d", "/c", "start", ""]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    command
        .arg(URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if command.status().await.is_ok_and(|status| status.success()) {
        Ok(())
    } else {
        Err("Could not open Claude usage in the default browser.".to_owned())
    }
}
