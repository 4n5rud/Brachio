//! The keep-awake commands. The hold itself lives in `brachio-power`.

use brachio_power::{KeepAwake, Support};
use serde::Serialize;
use tauri::State;

/// How the setting should look: offered, shown as unavailable with the reason, or hidden.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum KeepAwakeStatus {
    Supported,
    Unavailable { reason: String },
    Unsupported,
}

/// Whether this system can be kept awake. Checked on the machine, not by operating system name,
/// because Linux desktops differ in whether an inhibitor answers.
#[tauri::command]
pub async fn keep_awake_status() -> KeepAwakeStatus {
    let support = tauri::async_runtime::spawn_blocking(brachio_power::support)
        .await
        .unwrap_or(Support::Unsupported);
    match support {
        Support::Supported => KeepAwakeStatus::Supported,
        Support::Unavailable(reason) => KeepAwakeStatus::Unavailable { reason },
        Support::Unsupported => KeepAwakeStatus::Unsupported,
    }
}

/// Holds the system awake while `on` is true and releases the hold otherwise.
#[tauri::command]
pub fn keep_awake_set(keep: State<'_, KeepAwake>, on: bool) -> Result<(), String> {
    keep.set(on)
}
