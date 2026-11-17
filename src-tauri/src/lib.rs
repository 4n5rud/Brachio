//! App shell: window, tray, IPC commands. Domain logic stays in the crates.

mod terminal;
mod worktree;

use std::sync::Arc;

use brachio_pty::SessionManager;

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
        .manage(terminal::Terminals(Arc::new(SessionManager::default())))
        .invoke_handler(tauri::generate_handler![
            app_version,
            terminal::agents_detect,
            terminal::terminal_open,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            worktree::worktree_create,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Brachio");
}
