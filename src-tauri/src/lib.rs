//! App shell: window, tray, IPC commands. Domain logic stays in the crates.

mod terminal;
mod workspace;

use std::fs;
use std::sync::{Arc, Mutex};

use brachio_pty::SessionManager;
use tauri::{App, Manager};

/// The app version, for the about box.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Builds and runs the Tauri app.
///
/// # Panics
/// Panics if the Tauri runtime fails to start, which leaves nothing to show.
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(terminal::Terminals(Arc::new(SessionManager::default())))
        .setup(open_database)
        .invoke_handler(tauri::generate_handler![
            app_version,
            terminal::agents_detect,
            terminal::terminal_open,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            workspace::repo_list,
            workspace::repo_add,
            workspace::repo_remove,
            workspace::task_list,
            workspace::task_create,
            workspace::task_archive,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Brachio");
}

/// Opens `brachio.db` in the app's data folder and makes it app state.
fn open_database(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let folder = app.path().app_data_dir()?;
    fs::create_dir_all(&folder)?;
    let connection = brachio_store::open(&folder.join("brachio.db"))?;
    app.manage(workspace::Database(Mutex::new(connection)));
    Ok(())
}
