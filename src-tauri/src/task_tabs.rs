//! IPC for terminal identities, independent from process lifetime.

use std::collections::HashMap;

use brachio_store::{TabKind, TaskTab};
use serde::Serialize;
use tauri::State;

use crate::workspace::Database;

/// An open extra tab as the window sees it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    id: String,
    kind: &'static str,
    shell: Option<String>,
    name: Option<String>,
}

impl From<TaskTab> for TabInfo {
    fn from(tab: TaskTab) -> Self {
        Self {
            id: tab.tab_id,
            kind: tab.kind.as_str(),
            shell: tab.shell_id,
            name: None,
        }
    }
}

#[tauri::command]
pub fn task_open_tabs(
    database: State<'_, Database>,
    mut legacy: HashMap<String, String>,
) -> Result<HashMap<String, Vec<TabInfo>>, String> {
    legacy.retain(|_, tab| uuid::Uuid::parse_str(tab).is_ok());
    let mut connection = database.lock()?;
    let tabs = brachio_store::load_task_tabs(&mut connection, &legacy)
        .map_err(|error| error.to_string())?;
    let names = brachio_store::task_tab_names(&connection).map_err(|error| error.to_string())?;
    Ok(tabs
        .into_iter()
        .map(|(task, tabs)| {
            let tabs = tabs
                .into_iter()
                .map(|tab| {
                    let name = names.get(&tab.tab_id).cloned();
                    TabInfo {
                        name,
                        ..TabInfo::from(tab)
                    }
                })
                .collect();
            (task, tabs)
        })
        .collect())
}

#[tauri::command]
pub fn task_add_tab(
    database: State<'_, Database>,
    task_id: String,
    tab_id: String,
    kind: String,
    shell: Option<String>,
) -> Result<(), String> {
    if uuid::Uuid::parse_str(&tab_id).is_err() {
        return Err("Invalid terminal tab ID.".to_owned());
    }
    let kind = TabKind::parse(&kind).ok_or_else(|| "Unknown terminal tab kind.".to_owned())?;
    if shell.is_some() && kind != TabKind::Shell {
        return Err("Only a shell tab can name a shell.".to_owned());
    }
    if let Some(id) = shell.as_deref() {
        if crate::terminal_shells::find(id).is_none() {
            return Err("That shell is not installed.".to_owned());
        }
    }
    let connection = database.lock()?;
    brachio_store::add_task_tab(
        &connection,
        &task_id,
        &TaskTab {
            tab_id,
            kind,
            shell_id: shell,
        },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_close_tab(
    database: State<'_, Database>,
    task_id: String,
    tab_id: String,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::close_task_tab(&connection, &task_id, &tab_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_rename_tab(
    database: State<'_, Database>,
    task_id: String,
    tab_id: String,
    name: Option<String>,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::rename_task_tab(&connection, &task_id, &tab_id, name.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_reorder_tabs(
    database: State<'_, Database>,
    task_id: String,
    tab_ids: Vec<String>,
) -> Result<(), String> {
    let mut connection = database.lock()?;
    brachio_store::reorder_task_tabs(&mut connection, &task_id, &tab_ids)
        .map_err(|error| error.to_string())
}
