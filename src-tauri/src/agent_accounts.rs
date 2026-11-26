//! Native CLI authentication and read-only Codex quota reporting.

use std::env;
use std::future::Future;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use brachio_agent::{find_on_path, AgentKind, ResolvedCommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::timeout;

use crate::agent_account_env::{account_environment, config_variable, AccountsFolder};
use crate::workspace::{new_id, now_ms, Database};

const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
const QUOTA_TIMEOUT: Duration = Duration::from_secs(15);
const CACHE_TTL: Duration = Duration::from_secs(60);
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
    let mut command = Command::new(resolved.program);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .args(resolved.args)
        .envs(resolved.env)
        .envs(environment.iter().cloned())
        .kill_on_drop(true);
    Ok(command)
}

/// A completed read is shared with callers that arrived while it was running.
struct CachedReading<T> {
    completed_at: Instant,
    result: Result<T, String>,
}

/// Serializes CLI reads and keeps the most recent answer.
struct Cached<T>(AsyncMutex<Option<CachedReading<T>>>);

impl<T> Default for Cached<T> {
    fn default() -> Self {
        Self(AsyncMutex::new(None))
    }
}

impl<T: Clone> Cached<T> {
    async fn get(
        &self,
        force: bool,
        ttl: Duration,
        read: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let requested_at = Instant::now();
        let mut last = self.0.lock().await;
        if let Some(reading) = last.as_ref() {
            let finished_while_waiting = reading.completed_at > requested_at;
            let fresh = !force && reading.result.is_ok() && reading.completed_at.elapsed() < ttl;
            if finished_while_waiting || fresh {
                return reading.result.clone();
            }
        }
        let result = read.await;
        *last = Some(CachedReading {
            completed_at: Instant::now(),
            result: result.clone(),
        });
        result
    }

    async fn clear(&self) {
        *self.0.lock().await = None;
    }
}

/// Recent sign-in and Codex usage readings, shared by the status bar and the settings tab.
#[derive(Default)]
pub struct AgentReadings {
    codex_auth: Cached<AgentAuthStatus>,
    claude_auth: Cached<AgentAuthStatus>,
    codex_usage: Cached<CodexQuota>,
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
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentAuthStatus {
    signed_in: bool,
    account: Option<String>,
}

/// Reads sign-in status through the installed CLI; a reading under a minute old is reused unless
/// `force` is set.
#[tauri::command]
pub async fn agent_auth_status(
    kind: AgentKind,
    account_id: Option<String>,
    force: Option<bool>,
    readings: State<'_, AgentReadings>,
    database: State<'_, Database>,
    folder: State<'_, AccountsFolder>,
) -> Result<AgentAuthStatus, String> {
    if account_id.is_some() {
        let environment = selected_environment(&database, &folder, kind, account_id.as_deref())?;
        return read_auth_status(kind, &environment).await;
    }
    let cache = match kind {
        AgentKind::Codex => &readings.codex_auth,
        AgentKind::ClaudeCode => &readings.claude_auth,
        AgentKind::Grok => return Err("Grok signs in from its own terminal.".to_owned()),
    };
    cache
        .get(
            force.unwrap_or(false),
            CACHE_TTL,
            read_auth_status(kind, &[]),
        )
        .await
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
    readings: State<'_, AgentReadings>,
) -> Result<bool, String> {
    let kind = kind.unwrap_or(AgentKind::Codex);
    account_agent(kind)?;
    let environment = selected_environment(&database, &folder, kind, account_id.as_deref())?;
    let result = codex_auth_change(action, kind, environment, &run).await;
    readings.codex_auth.clear().await;
    readings.claude_auth.clear().await;
    readings.codex_usage.clear().await;
    result
}

async fn codex_auth_change(
    action: CodexAuthAction,
    kind: AgentKind,
    environment: Vec<(String, String)>,
    run: &SignInRun,
) -> Result<bool, String> {
    if matches!(action, CodexAuthAction::SignIn) {
        let args: &[&str] = if kind == AgentKind::ClaudeCode {
            &["auth", "login"]
        } else {
            &["login"]
        };
        return agent_sign_in(kind, args, environment, run).await;
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
    let mut command = std::process::Command::new(resolved.program);
    command
        .args(resolved.args)
        .envs(resolved.env)
        .envs(environment);
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
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    used_percent: f64,
    window_duration_mins: Option<i64>,
    resets_at: Option<i64>,
}

/// Usage windows for the active Codex account.
#[derive(Serialize, Deserialize, Clone)]
pub struct CodexQuota {
    #[serde(default, rename = "planType")]
    plan_type: Option<String>,
    primary: Option<QuotaWindow>,
    secondary: Option<QuotaWindow>,
}

/// Reads usage through the local Codex app-server protocol, without a model request.
#[tauri::command]
pub async fn codex_usage(
    account_id: Option<String>,
    force: Option<bool>,
    readings: State<'_, AgentReadings>,
    database: State<'_, Database>,
    folder: State<'_, AccountsFolder>,
) -> Result<CodexQuota, String> {
    let environment =
        selected_environment(&database, &folder, AgentKind::Codex, account_id.as_deref())?;
    if account_id.is_some() {
        return timeout(QUOTA_TIMEOUT, read_codex_usage(&environment))
            .await
            .map_err(|_| "Codex usage lookup timed out.".to_owned())?;
    }
    readings
        .codex_usage
        .get(force.unwrap_or(false), CACHE_TTL, async {
            timeout(QUOTA_TIMEOUT, read_codex_usage(&environment))
                .await
                .map_err(|_| "Codex usage lookup timed out.".to_owned())?
        })
        .await
}

async fn read_codex_usage(environment: &[(String, String)]) -> Result<CodexQuota, String> {
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
    use std::sync::atomic::AtomicUsize;

    async fn counted(reads: &AtomicUsize, answer: Result<u32, String>) -> Result<u32, String> {
        reads.fetch_add(1, Ordering::SeqCst);
        answer
    }

    #[tokio::test]
    async fn a_fresh_reading_is_reused() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        assert_eq!(
            cache.get(false, CACHE_TTL, counted(&reads, Ok(1))).await,
            Ok(1)
        );
        assert_eq!(
            cache.get(false, CACHE_TTL, counted(&reads, Ok(2))).await,
            Ok(1)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_expired_reading_is_read_again() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        cache
            .get(false, Duration::ZERO, counted(&reads, Ok(1)))
            .await
            .expect("read succeeds");
        let second = cache
            .get(false, Duration::ZERO, counted(&reads, Ok(2)))
            .await;
        assert_eq!(second, Ok(2));
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn force_skips_the_cache_and_replaces_it() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        cache
            .get(false, CACHE_TTL, counted(&reads, Ok(1)))
            .await
            .expect("read succeeds");
        assert_eq!(
            cache.get(true, CACHE_TTL, counted(&reads, Ok(2))).await,
            Ok(2)
        );
        assert_eq!(
            cache.get(false, CACHE_TTL, counted(&reads, Ok(3))).await,
            Ok(2)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn failures_are_not_kept() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        let failed = cache
            .get(false, CACHE_TTL, counted(&reads, Err("no".into())))
            .await;
        assert_eq!(failed, Err("no".to_owned()));
        assert_eq!(
            cache.get(false, CACHE_TTL, counted(&reads, Ok(5))).await,
            Ok(5)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn clear_drops_the_reading() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        cache
            .get(false, CACHE_TTL, counted(&reads, Ok(1)))
            .await
            .expect("read succeeds");
        cache.clear().await;
        assert_eq!(
            cache.get(false, CACHE_TTL, counted(&reads, Ok(2))).await,
            Ok(2)
        );
    }

    #[tokio::test]
    async fn simultaneous_requests_share_one_read() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        let slow = || async {
            reads.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(7)
        };
        let (a, b, c) = tokio::join!(
            cache.get(false, CACHE_TTL, slow()),
            cache.get(false, CACHE_TTL, slow()),
            cache.get(false, CACHE_TTL, slow()),
        );
        assert_eq!((a, b, c), (Ok(7), Ok(7), Ok(7)));
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn simultaneous_forced_requests_share_one_read() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        let slow = || async {
            reads.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(7)
        };
        let (a, b) = tokio::join!(
            cache.get(true, CACHE_TTL, slow()),
            cache.get(true, CACHE_TTL, slow()),
        );
        assert_eq!((a, b), (Ok(7), Ok(7)));
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn simultaneous_failed_requests_share_one_read() {
        let cache = Cached::<u32>::default();
        let reads = AtomicUsize::new(0);
        let slow = || async {
            reads.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(20)).await;
            Err("unavailable".to_owned())
        };
        let (a, b) = tokio::join!(
            cache.get(false, CACHE_TTL, slow()),
            cache.get(false, CACHE_TTL, slow()),
        );
        assert_eq!(
            (a, b),
            (Err("unavailable".to_owned()), Err("unavailable".to_owned()))
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(
            cache.get(false, CACHE_TTL, counted(&reads, Ok(9))).await,
            Ok(9)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }
}
