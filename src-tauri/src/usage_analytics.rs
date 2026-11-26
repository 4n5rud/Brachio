//! IPC for the usage analytics: scans the agents' local session logs into daily aggregates and
//! reports them. No network, and no money is estimated.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use brachio_agent::{providers, FileState, LogFile, UsageProvider};
use brachio_store::{LimitSnapshot, TaskRecord, UsageRow};
use serde::Serialize;
use tauri::{Manager, State};

use crate::agent_account_env::{existing_account_folders, AccountsFolder};
use crate::workspace::Database;

/// Most bytes read by one scan call. A larger backlog is finished by scanning again, so the page
/// can show what is read so far instead of waiting for all of it.
const SCAN_BYTE_BUDGET: u64 = 32 * 1024 * 1024;
const LIST_SIZE: usize = 8;

/// Keeps two scans from running at once.
#[derive(Default, Clone)]
pub struct UsageScanLock(Arc<Mutex<()>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    id: &'static str,
    name: &'static str,
    icon: &'static str,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    files_total: usize,
    files_read: usize,
    bytes_read: u64,
    /// The byte budget ran out; scanning again continues.
    truncated: bool,
}

#[derive(Serialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

impl Tokens {
    fn add(&mut self, row: &UsageRow) {
        self.input += row.input;
        self.output += row.output;
        self.cache_read += row.cache_read;
        self.cache_write += row.cache_write;
    }
    fn total(self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayRow {
    day: String,
    provider: String,
    #[serde(flatten)]
    tokens: Tokens,
    sessions: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareRow {
    key: String,
    name: String,
    #[serde(flatten)]
    tokens: Tokens,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageReport {
    daily: Vec<DayRow>,
    by_provider: Vec<ShareRow>,
    by_project: Vec<ShareRow>,
    by_task: Vec<ShareRow>,
    limits: Vec<LimitSnapshot>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn folder_name(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

#[tauri::command]
pub fn usage_providers() -> Vec<ProviderInfo> {
    providers()
        .iter()
        .map(|p| ProviderInfo {
            id: p.id(),
            name: p.display_name(),
            icon: p.icon(),
        })
        .collect()
}

/// The provider's logs: those of the login the CLI has on this computer, then those of each extra
/// account. A log reached through two folders is listed once, under the path seen first, so the
/// saved scan state of a log already scanned keeps its key.
fn usage_log_files(provider: &dyn UsageProvider, account_folders: &[PathBuf]) -> Vec<LogFile> {
    let mut seen = HashSet::new();
    provider
        .log_files()
        .into_iter()
        .chain(
            account_folders
                .iter()
                .flat_map(|folder| provider.logs_under(folder)),
        )
        .filter(|file| {
            seen.insert(
                file.path
                    .canonicalize()
                    .unwrap_or_else(|_| file.path.clone()),
            )
        })
        .collect()
}

fn scan_provider(
    provider: &dyn UsageProvider,
    database: &Database,
    files: Vec<LogFile>,
    report: &mut ScanReport,
) -> Result<(), String> {
    for file in files {
        report.files_total += 1;
        if report.bytes_read >= SCAN_BYTE_BUDGET {
            report.truncated = true;
            continue;
        }
        let key = file.path.to_string_lossy().into_owned();
        let saved = {
            let connection = database.lock()?;
            brachio_store::usage_file_state(&connection, &key).map_err(|error| error.to_string())?
        };
        let mut state = saved
            .and_then(|text| serde_json::from_str::<FileState>(&text).ok())
            .unwrap_or_default();
        let Ok(scanned) = provider.scan(&file, &mut state) else {
            continue;
        };
        report.bytes_read += scanned.bytes;
        if scanned.bytes == 0 {
            continue;
        }
        report.files_read += 1;
        let rows: Vec<UsageRow> = scanned
            .buckets
            .into_iter()
            .map(|bucket| UsageRow {
                provider: provider.id().to_owned(),
                day: bucket.day,
                project: bucket.project,
                session: bucket.session,
                input: bucket.usage.input,
                output: bucket.usage.output,
                cache_read: bucket.usage.cache_read,
                cache_write: bucket.usage.cache_write,
            })
            .collect();
        let text = serde_json::to_string(&state).map_err(|error| error.to_string())?;
        let mut connection = database.lock()?;
        brachio_store::usage_apply(&mut connection, provider.id(), &key, &text, &rows)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Reads the logs that grew since the last scan. `reset` forgets everything first (a manual rescan).
#[tauri::command]
pub async fn usage_scan(
    app: tauri::AppHandle,
    lock: State<'_, UsageScanLock>,
    reset: bool,
) -> Result<ScanReport, String> {
    let lock = lock.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let database = app.state::<Database>();
        let Ok(_running) = lock.0.try_lock() else {
            return Ok(ScanReport::default());
        };
        if reset {
            let mut connection = database.lock()?;
            brachio_store::usage_reset(&mut connection).map_err(|error| error.to_string())?;
        }
        let accounts = app.state::<AccountsFolder>();
        let mut report = ScanReport::default();
        for provider in providers() {
            let folders = {
                let connection = database.lock()?;
                existing_account_folders(&connection, &accounts.0, provider.id())?
            };
            let files = usage_log_files(provider.as_ref(), &folders);
            scan_provider(provider.as_ref(), &database, files, &mut report)?;
        }
        Ok(report)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn top(map: HashMap<String, (String, Tokens)>) -> Vec<ShareRow> {
    let mut rows: Vec<ShareRow> = map
        .into_iter()
        .map(|(key, (name, tokens))| ShareRow { key, name, tokens })
        .collect();
    rows.sort_by(|a, b| {
        b.tokens
            .total()
            .cmp(&a.tokens.total())
            .then_with(|| a.key.cmp(&b.key))
    });
    rows.truncate(LIST_SIZE);
    rows
}

/// Aggregates for the charts. `daily` covers `daily_since_day` on (so totals for today, the week and
/// the month can be added up); the lists cover `since_day` on and the chosen provider.
#[tauri::command]
pub async fn usage_report(
    app: tauri::AppHandle,
    provider: Option<String>,
    since_day: String,
    daily_since_day: String,
    limits_since_ms: i64,
) -> Result<UsageReport, String> {
    // A synchronous command would run on the main thread and wait behind a scan's writes.
    tauri::async_runtime::spawn_blocking(move || {
        build_report(
            &app.state::<Database>(),
            provider,
            since_day,
            daily_since_day,
            limits_since_ms,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

fn build_report(
    database: &Database,
    provider: Option<String>,
    since_day: String,
    daily_since_day: String,
    limits_since_ms: i64,
) -> Result<UsageReport, String> {
    let connection = database.lock()?;
    let provider = provider.as_deref();
    let first = if daily_since_day < since_day {
        &daily_since_day
    } else {
        &since_day
    };
    let rows = brachio_store::usage_rows(&connection, provider, first)
        .map_err(|error| error.to_string())?;

    let mut per_day: HashMap<(String, String), (Tokens, std::collections::HashSet<String>)> =
        HashMap::new();
    let mut by_provider: HashMap<String, (String, Tokens)> = HashMap::new();
    let mut by_project: HashMap<String, (String, Tokens)> = HashMap::new();
    let tasks = TaskRecord::list_open(&connection).map_err(|error| error.to_string())?;
    let task_of: HashMap<String, String> = tasks
        .iter()
        .map(|task| {
            (
                normalize(&task.worktree_path.to_string_lossy()),
                task.title.clone(),
            )
        })
        .collect();
    let mut by_task: HashMap<String, (String, Tokens)> = HashMap::new();
    let names: HashMap<&'static str, &'static str> = providers()
        .iter()
        .map(|p| (p.id(), p.display_name()))
        .collect();

    for row in &rows {
        let entry = per_day
            .entry((row.day.clone(), row.provider.clone()))
            .or_default();
        entry.0.add(row);
        entry.1.insert(row.session.clone());
        if row.day < since_day {
            continue;
        }
        let name = names
            .get(row.provider.as_str())
            .map_or_else(|| row.provider.clone(), |known| (*known).to_owned());
        by_provider
            .entry(row.provider.clone())
            .or_insert_with(|| (name, Tokens::default()))
            .1
            .add(row);
        if !row.project.is_empty() {
            by_project
                .entry(row.project.clone())
                .or_insert_with(|| (folder_name(&row.project), Tokens::default()))
                .1
                .add(row);
            if let Some(title) = task_of.get(&normalize(&row.project)) {
                by_task
                    .entry(normalize(&row.project))
                    .or_insert_with(|| (title.clone(), Tokens::default()))
                    .1
                    .add(row);
            }
        }
    }
    let mut daily: Vec<DayRow> = per_day
        .into_iter()
        .map(|((day, provider), (tokens, sessions))| DayRow {
            day,
            provider,
            tokens,
            sessions: sessions.len(),
        })
        .collect();
    daily.sort_by(|a, b| a.day.cmp(&b.day).then_with(|| a.provider.cmp(&b.provider)));
    let limits = brachio_store::limit_history(&connection, limits_since_ms)
        .map_err(|error| error.to_string())?;
    Ok(UsageReport {
        daily,
        by_provider: top(by_provider),
        by_project: top(by_project),
        by_task: top(by_task),
        limits,
    })
}

/// Records a plan limit reading for the history line; readings of one window are 30 minutes apart at least.
#[tauri::command]
pub fn usage_limit_snapshot(
    database: State<'_, Database>,
    provider: String,
    window: String,
    used_percent: f64,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::record_limit_snapshot(&connection, &provider, &window, now_ms(), used_percent)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use brachio_agent::{claude_logs_in, ClaudeProvider, Scanned};
    use std::path::Path;

    /// Claude Code with its built-in login in a temporary folder instead of the real home.
    struct Claude(PathBuf);

    impl UsageProvider for Claude {
        fn id(&self) -> &'static str {
            "claude_code"
        }
        fn display_name(&self) -> &'static str {
            "Claude Code"
        }
        fn icon(&self) -> &'static str {
            "claude"
        }
        fn log_files(&self) -> Vec<LogFile> {
            claude_logs_in(&self.0.join("projects"))
        }
        fn logs_under(&self, config_root: &Path) -> Vec<LogFile> {
            ClaudeProvider.logs_under(config_root)
        }
        fn scan(&self, file: &LogFile, state: &mut FileState) -> std::io::Result<Scanned> {
            ClaudeProvider.scan(file, state)
        }
    }

    fn session_log(config: &Path, session: &str, output: u64) {
        let folder = config.join("projects").join("p");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join(format!("{session}.jsonl")),
            format!(
                r#"{{"timestamp":"2026-11-01T10:00:00Z","cwd":"/work","message":{{"id":"{session}","usage":{{"input_tokens":0,"output_tokens":{output}}}}}}}"#
            ) + "\n",
        )
        .unwrap();
    }

    fn total_output(database: &Database) -> u64 {
        let connection = database.lock().unwrap();
        brachio_store::usage_rows(&connection, Some("claude_code"), "2026-01-01")
            .unwrap()
            .iter()
            .map(|row| row.output)
            .sum()
    }

    #[test]
    fn logs_of_every_extra_account_are_counted_once() {
        let built_in = tempfile::tempdir().unwrap();
        let accounts = tempfile::tempdir().unwrap();
        session_log(built_in.path(), "home", 5);
        session_log(&accounts.path().join("c1"), "work", 7);
        session_log(&accounts.path().join("c2"), "side", 11);
        // Sign-in files sit next to the logs; only session logs are listed.
        std::fs::write(accounts.path().join("c1").join(".credentials.json"), "{}").unwrap();
        session_log(&accounts.path().join("x1"), "codex", 1000);

        let connection = brachio_store::open_in_memory().unwrap();
        for (id, agent) in [
            ("c1", "claude_code"),
            ("c2", "claude_code"),
            ("x1", "codex"),
        ] {
            brachio_store::add_agent_account(&connection, id, agent, id, 1).unwrap();
        }
        #[cfg(unix)]
        {
            brachio_store::add_agent_account(&connection, "c3", "claude_code", "c3", 2).unwrap();
            std::os::unix::fs::symlink(accounts.path().join("c1"), accounts.path().join("c3"))
                .unwrap();
        }
        let database = Database(Mutex::new(connection));
        let provider = Claude(built_in.path().to_path_buf());

        for _ in 0..2 {
            let mut folders = {
                let connection = database.lock().unwrap();
                existing_account_folders(&connection, accounts.path(), "claude_code").unwrap()
            };
            // The built-in folder reached again, as when the app runs with it as an account.
            folders.push(built_in.path().to_path_buf());
            let files = usage_log_files(&provider, &folders);
            assert!(files
                .iter()
                .all(|file| file.path.extension().is_some_and(|ext| ext == "jsonl")));
            scan_provider(&provider, &database, files, &mut ScanReport::default()).unwrap();
        }
        assert_eq!(total_output(&database), 5 + 7 + 11);
    }
}
