//! App shell: window, tray, IPC commands. Domain logic stays in the crates.

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Builds and runs the Tauri app.
///
/// # Panics
/// Panics if the Tauri runtime fails to start, which leaves nothing to show.
#[allow(clippy::expect_used)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("failed to start Brachio");
}
