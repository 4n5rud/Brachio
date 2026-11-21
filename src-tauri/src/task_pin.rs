//! IPC for the tasks pinned to the top of the sidebar.

use tauri::State;

use crate::workspace::Database;

#[tauri::command]
pub fn task_pins_list(database: State<'_, Database>) -> Result<Vec<String>, String> {
    let connection = database.lock()?;
    brachio_store::pinned_task_ids(&connection).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_pin_set(
    database: State<'_, Database>,
    task_id: String,
    pinned: bool,
    at: i64,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::set_task_pinned(&connection, &task_id, pinned, at)
        .map_err(|error| error.to_string())
}
