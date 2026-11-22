//! IPC for the reusable prompts and shell commands of a project.

use tauri::State;
use uuid::Uuid;

use brachio_store::ProjectCue;

use crate::workspace::Database;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
}

#[tauri::command]
pub fn cue_list(database: State<'_, Database>, repo_id: String) -> Result<Vec<ProjectCue>, String> {
    let connection = database.lock()?;
    brachio_store::project_cues(&connection, &repo_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cue_add(
    database: State<'_, Database>,
    repo_id: String,
    kind: String,
    label: String,
    body: String,
) -> Result<ProjectCue, String> {
    let cue = ProjectCue { id: Uuid::new_v4().to_string(), repo_id, kind, label, body };
    let connection = database.lock()?;
    brachio_store::add_project_cue(&connection, &cue, now_ms()).map_err(|error| error.to_string())?;
    Ok(cue)
}

#[tauri::command]
pub fn cue_delete(database: State<'_, Database>, id: String) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::delete_project_cue(&connection, &id).map_err(|error| error.to_string())
}
