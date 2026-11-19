//! IPC for terminal identities, independent from process lifetime.

use std::collections::HashMap;

use tauri::State;

use crate::workspace::Database;

#[tauri::command]
pub fn task_secondary_tabs(
    database: State<'_, Database>,
    mut legacy: HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    legacy.retain(|_, tab| uuid::Uuid::parse_str(tab).is_ok());
    let mut connection = database.lock()?;
    brachio_store::load_secondary_tabs(&mut connection, &legacy).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_set_secondary_tab(
    database: State<'_, Database>,
    task_id: String,
    tab_id: Option<String>,
) -> Result<(), String> {
    if tab_id
        .as_ref()
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
    {
        return Err("Invalid terminal tab ID.".to_owned());
    }
    let connection = database.lock()?;
    brachio_store::set_secondary_tab(&connection, &task_id, tab_id.as_deref())
        .map_err(|error| error.to_string())
}
