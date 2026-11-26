//! Native CLI authentication and read-only Codex quota reporting.

use std::env;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use brachio_agent::{find_on_path, AgentKind, ResolvedCommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::agent_account_env::{account_environment, config_variable, AccountsFolder};
use crate::workspace::{new_id, now_ms, Database};

const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
const QUOTA_TIMEOUT: Duration = Duration::from_secs(15);
const CODEX_ACCOUNT_TIMEOUT: Duration = Duration::from_millis(500);
const PROTOCOL_LIMIT: u64 = 512 * 1024;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn resolved_cli(kind: AgentKind, args: &[&str]) -> Result<ResolvedCommand, String> {
    let path = env::var_os("PATH");
    let extensions = env::var_os("PATHEXT");
    let executable = find_on_path(kind.program(), path.as_deref(), extensions.as_deref())
        .ok_or_else(|| format!("{} is not installed or not on PATH", kind.program()))?;
    let args: Vec<_> = args.iter().map(|arg| (*arg).to_owned()).collect();
    ResolvedCommand::new(&executable, &args).map_err(|error| error.to_string())
}

fn cli(
    kind: AgentKind,
    args: &[&str],
    environment: &[(String, String)],
) -> Result<Command, String> {
    let resolved = resolved_cli(kind, args)?;
    let mut command = Command::from(configured_cli(resolved, environment));
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command.kill_on_drop(true);
    Ok(command)
}

fn configured_cli(
    resolved: ResolvedCommand,
    environment: &[(String, String)],
) -> std::process::Command {
    let mut command = std::process::Command::new(resolved.program);
    command
        .args(resolved.args)
        .envs(resolved.env)
        .envs(environment.iter().cloned());
    command
}

/// One registered login in the account picker.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountChoice {
    id: Option<String>,
    name: String,
    is_default: bool,
}

fn account_agent(kind: AgentKind) -> Result<&'static str, String> {
    config_variable(kind)
        .map(|_| kind.spec().id)
        .ok_or_else(|| "Grok signs in from its own terminal.".to_owned())
}

fn get_account(
    db: &rusqlite::Connection,
    kind: AgentKind,
    id: &str,
) -> Result<brachio_store::AgentAccount, String> {
    let agent = account_agent(kind)?;
    let account = brachio_store::agent_account(db, id).map_err(|error| error.to_string())?;
    if account.agent != agent {
        return Err("The account belongs to another agent.".to_owned());
    }
    Ok(account)
}

fn selected_environment(
    database: &Database,
    folder: &AccountsFolder,
    kind: AgentKind,
    account_id: Option<&str>,
) -> Result<Vec<(String, String)>, String> {
    let Some(id) = account_id else {
        return Ok(Vec::new());
    };
    let account = get_account(&*database.lock()?, kind, id)?;
    account_environment(&folder.0, kind, &account)
}

/// Lists the built-in login and the extra accounts for an agent.
#[tauri::command]
pub fn agent_accounts_list(
    agent: AgentKind,
    database: State<'_, Database>,
) -> Result<Vec<AccountChoice>, String> {
    let accounts = brachio_store::agent_accounts(&*database.lock()?, account_agent(agent)?)
        .map_err(|error| error.to_string())?;
    let builtin = AccountChoice {
        id: None,
        name: "Built-in".to_owned(),
        is_default: !accounts.iter().any(|account| account.is_default),
    };
    Ok(std::iter::once(builtin)
        .chain(accounts.into_iter().map(|account| AccountChoice {
            id: Some(account.id),
            name: account.name,
            is_default: account.is_default,
        }))
        .collect())
}

/// Registers an extra login with its own CLI configuration.
#[tauri::command]
pub fn agent_account_add(
    agent: AgentKind,
    name: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    brachio_store::add_agent_account(
        &*database.lock()?,
        &new_id(),
        account_agent(agent)?,
        &name,
        now_ms(),
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// Changes an extra account’s display name.
#[tauri::command]
pub fn agent_account_rename(
    agent: AgentKind,
    account_id: String,
    name: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    let db = database.lock()?;
    get_account(&db, agent, &account_id)?;
    brachio_store::rename_agent_account(&db, &account_id, &name).map_err(|error| error.to_string())
}

/// Removes an extra account from Brachio.
#[tauri::command]
pub fn agent_account_remove(
    agent: AgentKind,
    account_id: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    let db = database.lock()?;
    get_account(&db, agent, &account_id)?;
    brachio_store::remove_agent_account(&db, &account_id).map_err(|error| error.to_string())
}

/// Changes which login new tasks use by default.
#[tauri::command]
pub fn agent_account_set_default(
    agent: AgentKind,
    account_id: Option<String>,
    database: State<'_, Database>,
) -> Result<(), String> {
    brachio_store::set_default_agent_account(
        &*database.lock()?,
        account_agent(agent)?,
        account_id.as_deref(),
    )
    .map_err(|error| error.to_string())
}

/// Only account metadata intended for display is returned.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAuthStatus {
    signed_in: bool,
    account: Option<String>,
}

/// The login pinned to a task, without credential or provider metadata.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAccountInfo {
    agent: AgentKind,
    name: Option<String>,
    source: TaskAccountSource,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum TaskAccountSource {
    Shared,
    Isolated,
    Removed,
}

fn account_info(
    db: &rusqlite::Connection,
    task_id: &str,
) -> Result<Option<TaskAccountInfo>, String> {
    use brachio_store::{StoreError, TaskRecord};

    let task = TaskRecord::get(db, task_id).map_err(|error| error.to_string())?;
    let agent = match task.agent_kind.as_deref() {
        Some("codex") => AgentKind::Codex,
        Some("claude_code") => AgentKind::ClaudeCode,
        _ => return Ok(None),
    };
    let (name, source) = if let Some(id) =
        brachio_store::task_agent_account(db, task_id).map_err(|error| error.to_string())?
    {
        match brachio_store::agent_account(db, &id) {
            Ok(account) if account.agent == agent.spec().id => {
                (Some(account.name), TaskAccountSource::Isolated)
            }
            Ok(_) | Err(StoreError::NotFound { .. }) => (None, TaskAccountSource::Removed),
            Err(error) => return Err(error.to_string()),
        }
    } else {
        (None, TaskAccountSource::Shared)
    };
    Ok(Some(TaskAccountInfo {
        agent,
        name,
        source,
    }))
}

/// Reports the account a saved task uses.
#[tauri::command]
pub fn task_agent_account_info(
    task_id: String,
    database: State<'_, Database>,
) -> Result<Option<TaskAccountInfo>, String> {
    account_info(&*database.lock()?, &task_id)
}

/// Reads sign-in status through the installed CLI.
#[tauri::command]
pub async fn agent_auth_status(
    kind: AgentKind,
    account_id: Option<String>,
    database: State<'_, Database>,
    folder: State<'_, AccountsFolder>,
) -> Result<AgentAuthStatus, String> {
    let environment = selected_environment(&database, &folder, kind, account_id.as_deref())?;
    read_auth_status(kind, &environment).await
}

/// Requires an authenticated isolated login before a new agent process starts.
pub async fn check_isolated_login(
    kind: Option<AgentKind>,
    environment: &[(String, String)],
) -> Result<(), String> {
    let Some(kind) = kind.filter(|_| !environment.is_empty()) else {
        return Ok(());
    };
    if matches!(read_auth_status(kind, environment).await, Ok(status) if status.signed_in) {
        return Ok(());
    }
    Err("ISOLATED_LOGIN_REQUIRED: Sign in to this Brachio-only account in Settings > Agents, then retry.".to_owned())
}

async fn read_auth_status(
    kind: AgentKind,
    environment: &[(String, String)],
) -> Result<AgentAuthStatus, String> {
    let args: &[&str] = match kind {
        AgentKind::Codex => &["login", "status"],
        AgentKind::ClaudeCode => &["auth", "status", "--json"],
        AgentKind::Grok => return Err("Grok signs in from its own terminal.".to_owned()),
    };
    let output = timeout(
        STATUS_TIMEOUT,
        cli(kind, args, environment)?
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
        AgentKind::Grok => Err("Grok signs in from its own terminal.".to_owned()),
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

/// The browser sign-in the app started, if one is running, so it can be cancelled. Only one runs
/// at a time.
#[derive(Default)]
pub struct SignInRun(Mutex<Option<Arc<AtomicBool>>>);

impl SignInRun {
    fn begin(&self) -> Result<Arc<AtomicBool>, String> {
        let mut running = self.0.lock().map_err(|error| error.to_string())?;
        if running.is_some() {
            return Err("A sign-in is already running.".to_owned());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *running = Some(Arc::clone(&cancel));
        Ok(cancel)
    }

    fn finish(&self) {
        if let Ok(mut running) = self.0.lock() {
            *running = None;
        }
    }

    /// Asks the running sign-in to stop; false when none is running.
    fn cancel(&self) -> bool {
        let running = self.0.lock().ok();
        running
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .is_some_and(|cancel| {
                cancel.store(true, Ordering::Release);
                true
            })
    }
}

/// Runs the explicitly selected vendor CLI command. Credentials stay with the CLI.
///
/// Resolves to `false` when a sign-in was cancelled, `true` otherwise.
#[tauri::command]
pub async fn codex_auth_action(
    action: CodexAuthAction,
    kind: Option<AgentKind>,
    account_id: Option<String>,
    database: State<'_, Database>,
    folder: State<'_, AccountsFolder>,
    run: State<'_, SignInRun>,
) -> Result<bool, String> {
    let kind = kind.unwrap_or(AgentKind::Codex);
    account_agent(kind)?;
    let environment = selected_environment(&database, &folder, kind, account_id.as_deref())?;
    if matches!(action, CodexAuthAction::SignIn) {
        let args: &[&str] = if kind == AgentKind::ClaudeCode {
            &["auth", "login"]
        } else {
            &["login"]
        };
        return agent_sign_in(kind, args, environment, &run).await;
    }
    let args: &[&str] = if kind == AgentKind::ClaudeCode {
        &["auth", "logout"]
    } else {
        &["logout"]
    };
    let status = timeout(
        STATUS_TIMEOUT,
        cli(kind, args, &environment)?
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
    )
    .await
    .map_err(|_| "Agent authentication timed out. Retry from the agent CLI.".to_owned())?
    .map_err(|_| "Could not start agent authentication.".to_owned())?;
    if status.success() {
        Ok(true)
    } else {
        Err("Agent authentication did not complete. Check the agent CLI.".to_owned())
    }
}

/// The vendor login CLI runs inside its own process tree, so cancelling can stop
/// the CLI and anything it started.
async fn agent_sign_in(
    kind: AgentKind,
    args: &[&str],
    environment: Vec<(String, String)>,
    run: &SignInRun,
) -> Result<bool, String> {
    let resolved = resolved_cli(kind, args)?;
    let mut command = configured_cli(resolved, &environment);
    // A Windows console lets the user also see the CLI's fallback instructions.
    if !cfg!(windows) {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    let cancel = run.begin()?;
    let ended = tauri::async_runtime::spawn_blocking(move || {
        brachio_process::run_tree_cancellable(command, true, LOGIN_TIMEOUT, &cancel)
    })
    .await;
    run.finish();
    match ended.map_err(|error| error.to_string())? {
        Ok(brachio_process::Ended::Exited(status)) if status.success() => Ok(true),
        Ok(brachio_process::Ended::Exited(_)) => {
            Err("Agent authentication did not complete. Check the agent CLI.".to_owned())
        }
        Ok(brachio_process::Ended::Cancelled) => Ok(false),
        Ok(brachio_process::Ended::TimedOut) => {
            Err("Agent authentication timed out. Retry from the agent CLI.".to_owned())
        }
        Err(_) => Err("Could not start agent authentication.".to_owned()),
    }
}

/// Stops the browser sign-in the app started and its helper processes. The browser tab the CLI
/// opened stays open; with the CLI gone, its sign-in page has nothing to finish.
#[tauri::command]
pub fn codex_auth_cancel(run: State<'_, SignInRun>) -> bool {
    run.cancel()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account: Option<String>,
    #[serde(default, rename = "planType")]
    plan_type: Option<String>,
    primary: Option<QuotaWindow>,
    secondary: Option<QuotaWindow>,
}

/// Reads usage through the local Codex app-server protocol, without a model request.
#[tauri::command]
pub async fn codex_usage(
    account_id: Option<String>,
    include_account: Option<bool>,
    database: State<'_, Database>,
    folder: State<'_, AccountsFolder>,
) -> Result<CodexQuota, String> {
    let environment =
        selected_environment(&database, &folder, AgentKind::Codex, account_id.as_deref())?;
    timeout(
        QUOTA_TIMEOUT,
        read_codex_usage(&environment, include_account.unwrap_or(false)),
    )
    .await
    .map_err(|_| "Codex usage lookup timed out.".to_owned())?
}

async fn read_codex_usage(
    environment: &[(String, String)],
    include_account: bool,
) -> Result<CodexQuota, String> {
    let unavailable = || "Codex did not return usage for this login.".to_owned();
    let mut child = cli(AgentKind::Codex, &["app-server"], environment)?
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
    let account = if include_account {
        let read = async {
            send(
                &mut input,
                &json!({ "id": 2, "method": "account/read", "params": { "refreshToken": false } }),
            )
            .await?;
            receive(&mut lines, 2).await
        };
        timeout(CODEX_ACCOUNT_TIMEOUT, read)
            .await
            .ok()
            .and_then(Result::ok)
            .and_then(|metadata| metadata["account"]["email"].as_str().map(str::to_owned))
    } else {
        None
    };

    send(
        &mut input,
        &json!({ "id": 3, "method": "account/rateLimits/read", "params": null }),
    )
    .await?;
    let usage = receive(&mut lines, 3).await;
    // Closing stdio first lets npm's Windows launcher and its child exit together.
    drop(input);
    drop(lines);
    let _ = timeout(Duration::from_millis(500), child.wait()).await;
    match usage.and_then(|mut reply| {
        serde_json::from_value::<CodexQuota>(reply["rateLimits"].take()).map_err(|_| unavailable())
    }) {
        Ok(mut quota) => {
            quota.account = account;
            Ok(quota)
        }
        Err(_) if account.is_some() => Ok(CodexQuota {
            account,
            plan_type: None,
            primary: None,
            secondary: None,
        }),
        Err(error) => Err(error),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn isolated_cli_login_status_and_logout_leave_other_logins_untouched() {
        use crate::agent_account_env::account_environment;

        let folders = tempfile::tempdir().expect("temporary accounts");
        let script = folders.path().join("fake-cli.sh");
        std::fs::write(
            &script,
            r#"set -eu
variable="$1"
action="$2"
fallback="$3"
case "$variable" in
  CODEX_HOME) config="${CODEX_HOME:-$fallback}" ;;
  CLAUDE_CONFIG_DIR) config="${CLAUDE_CONFIG_DIR:-$fallback}" ;;
esac
case "$action" in
  login) printf 'signed-in' > "$config/login-marker" ;;
  status) test -f "$config/login-marker" ;;
  logout) rm -f "$config/login-marker" ;;
esac
"#,
        )
        .expect("fake CLI script");
        let db = brachio_store::open_in_memory().expect("database");
        for (kind, variable) in [
            (AgentKind::Codex, "CODEX_HOME"),
            (AgentKind::ClaudeCode, "CLAUDE_CONFIG_DIR"),
        ] {
            let agent = kind.spec().id;
            let id_prefix = agent.replace('_', "-");
            let builtin = folders.path().join(format!("{agent}-builtin"));
            std::fs::create_dir(&builtin).expect("built-in config");
            let chosen = brachio_store::add_agent_account(
                &db,
                &format!("{id_prefix}-chosen"),
                agent,
                "Chosen",
                1,
            )
            .expect("chosen account");
            let other = brachio_store::add_agent_account(
                &db,
                &format!("{id_prefix}-other"),
                agent,
                "Other",
                2,
            )
            .expect("other account");
            let chosen_env =
                account_environment(folders.path(), kind, &chosen).expect("chosen environment");
            let other_env =
                account_environment(folders.path(), kind, &other).expect("other environment");
            let run = |action: &str, environment: &[(String, String)]| {
                // The inherited home deliberately points at the built-in account. The selected
                // account must override it for every status and authentication operation.
                let resolved = ResolvedCommand {
                    program: "/bin/sh".into(),
                    args: vec![
                        script.to_string_lossy().into_owned(),
                        variable.to_owned(),
                        action.to_owned(),
                        builtin.to_string_lossy().into_owned(),
                    ],
                    env: vec![(variable.to_owned(), builtin.to_string_lossy().into_owned())],
                };
                configured_cli(resolved, environment)
                    .status()
                    .expect("run fake CLI")
                    .success()
            };
            assert!(run("login", &[]));
            assert!(run("login", &other_env));
            assert!(
                !run("status", &chosen_env),
                "a fresh isolated home must not inherit built-in login"
            );
            assert!(run("login", &chosen_env));
            assert!(run("status", &chosen_env));
            assert!(run("logout", &chosen_env));
            assert!(!run("status", &chosen_env));
            assert!(run("status", &other_env), "another isolated login survives");
            assert!(run("status", &[]), "built-in login survives");
            assert_eq!(
                std::fs::read_to_string(builtin.join("login-marker")).expect("built-in marker"),
                "signed-in"
            );
        }
    }

    #[test]
    fn a_task_reports_its_saved_login_source_even_after_removal() {
        let db = brachio_store::open_in_memory().expect("database");
        db.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, agent_kind)
             VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0, 'codex');",
        )
        .expect("task");

        let read = || {
            account_info(&db, "t")
                .expect("info")
                .expect("Codex account")
        };
        assert!(matches!(read().source, TaskAccountSource::Shared));
        brachio_store::add_agent_account(&db, "x", "codex", "Work", 1).expect("add account");
        brachio_store::set_task_agent_account(&db, "t", "x").expect("set task account");
        let info = read();
        assert!(matches!(info.source, TaskAccountSource::Isolated));
        assert_eq!(info.name.as_deref(), Some("Work"));
        brachio_store::remove_agent_account(&db, "x").expect("remove account");
        assert!(matches!(read().source, TaskAccountSource::Removed));
    }
}
