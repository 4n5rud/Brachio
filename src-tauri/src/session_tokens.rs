//! IPC for the token totals of each task's agent session, read from the agent's own local log.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use brachio_agent::{claude_log_path, codex_rollout_path, LogFormat, TokenUsage, UsageTracker};
use brachio_store::TaskRecord;
use tauri::State;

use crate::workspace::Database;

/// Most tasks read in one call.
const MAX_TASKS: usize = 64;

/// One incremental reader per task, kept so a refresh reads only what was appended.
#[derive(Default, Clone)]
pub struct TokenTrackers(Arc<Mutex<HashMap<String, UsageTracker>>>);

fn locate(task: &TaskRecord) -> Option<(LogFormat, std::path::PathBuf)> {
    let session = task.agent_session_id.as_deref();
    match task.agent_kind.as_deref()? {
        "claude_code" => Some((
            LogFormat::Claude,
            claude_log_path(&task.worktree_path, session)?,
        )),
        "codex" => Some((
            LogFormat::Codex,
            codex_rollout_path(&task.worktree_path, session)?,
        )),
        _ => None,
    }
}

fn read_all(tasks: Vec<TaskRecord>, trackers: &TokenTrackers) -> HashMap<String, TokenUsage> {
    let mut found = HashMap::new();
    let Ok(mut guard) = trackers.0.lock() else {
        return found;
    };
    for task in tasks {
        let Some((format, path)) = locate(&task) else {
            guard.remove(&task.id);
            continue;
        };
        let tracker = guard
            .entry(task.id.clone())
            .and_modify(|tracker| {
                if tracker.path() != path {
                    *tracker = UsageTracker::new(format, path.clone());
                }
            })
            .or_insert_with(|| UsageTracker::new(format, path));
        if let Ok(usage) = tracker.refresh() {
            found.insert(task.id, usage);
        }
    }
    found
}

/// Token totals per task id. A task whose log is missing or unreadable is left out, so the app
/// shows nothing for it.
#[tauri::command]
pub async fn tasks_token_usage(
    database: State<'_, Database>,
    trackers: State<'_, TokenTrackers>,
    ids: Vec<String>,
) -> Result<HashMap<String, TokenUsage>, String> {
    let tasks: Vec<TaskRecord> = {
        let connection = database.lock()?;
        ids.iter()
            .take(MAX_TASKS)
            .filter_map(|id| TaskRecord::get(&connection, id).ok())
            .collect()
    };
    let trackers = trackers.inner().clone();
    tauri::async_runtime::spawn_blocking(move || read_all(tasks, &trackers))
        .await
        .map_err(|error| error.to_string())
}
