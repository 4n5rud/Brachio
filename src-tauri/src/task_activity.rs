//! IPC for the per-task activity history shown in the inspector.

use serde::Serialize;
use tauri::State;

use crate::workspace::Database;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityView {
    id: i64,
    kind: String,
    detail: Option<String>,
    at: i64,
}

#[tauri::command]
pub fn task_activity_list(
    database: State<'_, Database>,
    task_id: String,
) -> Result<Vec<ActivityView>, String> {
    let connection = database.lock()?;
    brachio_store::task_activity(&connection, &task_id)
        .map(|entries| {
            entries
                .into_iter()
                .map(|entry| ActivityView {
                    id: entry.id,
                    kind: entry.kind,
                    detail: entry.detail,
                    at: entry.created_at_ms,
                })
                .collect()
        })
        .map_err(|error| error.to_string())
}

/// Records an event only the interface sees, such as an agent's status or an answered approval.
#[tauri::command]
pub fn task_activity_record(
    database: State<'_, Database>,
    task_id: String,
    kind: String,
    detail: Option<String>,
    at: i64,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::record_activity(&connection, &task_id, &kind, detail.as_deref(), at)
        .map_err(|error| error.to_string())
}
