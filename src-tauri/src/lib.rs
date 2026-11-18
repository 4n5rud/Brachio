//! App shell: window, tray, IPC commands. Domain logic stays in the crates.

mod runtime_client;
mod runtime_terminals;
mod terminal;
mod workspace;

use std::fs;
use std::sync::{Arc, Mutex};

use brachio_pty::SessionManager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
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
        .manage(runtime_terminals::RuntimeTerminals::default())
        .setup(|app| {
            open_database(app)?;
            if let Err(error) = install_tray(app) {
                tracing::warn!(%error, "could not install tray");
            }
            Ok(())
        })
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
            workspace::task_list_archived,
            workspace::task_restore,
            workspace::task_delete_preview,
            workspace::task_delete,
            workspace::task_files,
            workspace::task_changes,
            workspace::task_git_action,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Brachio");
}

fn install_tray(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Brachio", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Brachio", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("Brachio")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let result = window
                        .show()
                        .and_then(|()| window.unminimize())
                        .and_then(|()| window.set_focus());
                    if let Err(error) = result {
                        tracing::warn!(%error, "could not show the main window");
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Opens `brachio.db` in the app's data folder and makes it app state.
fn open_database(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let folder = app.path().app_data_dir()?;
    fs::create_dir_all(&folder)?;
    let connection = brachio_store::open(&folder.join("brachio.db"))?;
    app.manage(workspace::Database(Mutex::new(connection)));
    Ok(())
}
