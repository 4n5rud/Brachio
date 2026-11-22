//! IPC for the per-project history of prompts.

use tauri::State;

use brachio_store::PromptHistoryEntry;

use crate::workspace::Database;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[tauri::command]
pub fn prompt_history_list(
    database: State<'_, Database>,
    repo_id: String,
) -> Result<Vec<PromptHistoryEntry>, String> {
    let connection = database.lock()?;
    brachio_store::prompt_history(&connection, &repo_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn prompt_history_add(
    database: State<'_, Database>,
    repo_id: String,
    task_id: Option<String>,
    kind: String,
    body: String,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::add_prompt_history(
        &connection,
        &repo_id,
        task_id.as_deref(),
        &kind,
        &body,
        now_ms(),
    )
    .map_err(|error| error.to_string())
}
