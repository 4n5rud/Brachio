//! Quitting from the window: the runtime keeps terminals alive across a quit, so a quit only
//! stops the programs the person asked to stop.

use brachio_runtime::client::{Client, ClientError};
use brachio_runtime::wire::Action;
use tauri::{AppHandle, State};

use crate::runtime_client;
use crate::terminal_authorization;
use crate::workspace::Database;

/// Stops the programs behind these terminal keys in the current runtime and any older one still up.
/// A key nothing hosts is skipped, and one failure never stops the rest.
pub(crate) async fn stop_keys(keys: &[String]) {
    if !runtime_client::enabled() {
        return;
    }
    let mut clients = vec![Client::new(&runtime_client::endpoint())];
    clients.extend(runtime_client::older_clients().await);
    for key in keys {
        for client in &clients {
            match client.call(Action::Stop { key: key.clone() }).await {
                Ok(_) | Err(ClientError::NotRunning | ClientError::Refused(_)) => {}
                Err(error) => tracing::warn!(%error, "could not stop a terminal before quitting"),
            }
        }
    }
}

/// Keeps only keys that belong to a registered task terminal.
fn task_keys(database: &Database, keys: Vec<String>) -> Result<Vec<String>, String> {
    let connection = database.lock()?;
    Ok(keys
        .into_iter()
        .filter(|key| terminal_authorization::resolve(&connection, key).is_ok())
        .collect())
}

/// Stops the named task terminals and leaves the app running.
#[tauri::command]
pub async fn terminals_stop(
    database: State<'_, Database>,
    keys: Vec<String>,
) -> Result<(), String> {
    let keys = task_keys(&database, keys)?;
    stop_keys(&keys).await;
    Ok(())
}

/// Stops the named task terminals, then quits. An empty list quits and leaves everything running.
#[tauri::command]
pub async fn app_quit(
    app: AppHandle,
    database: State<'_, Database>,
    stop: Vec<String>,
) -> Result<(), String> {
    let keys = task_keys(&database, stop)?;
    stop_keys(&keys).await;
    app.exit(0);
    Ok(())
}
