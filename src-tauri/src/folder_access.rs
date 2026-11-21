//! Recognizes the failure macOS gives when it blocks an app from reading a folder until the
//! user allows it (Documents, Desktop, Downloads, removable volumes) in Privacy and Security.

use std::path::Path;

const MARKER: &str = "folder access: ";

/// Whether `text` is the "Operation not permitted" failure (EPERM) that macOS uses for a blocked
/// folder. Plain permission errors ("Permission denied", EACCES) are a different problem.
#[must_use]
pub fn is_blocked(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("operation not permitted") || lower.contains("(os error 1)")
}

/// `folder access: <path>` for the interface when `message` says macOS blocked `path`. Only macOS
/// blocks folders this way, so on other systems the message stays as it is.
#[must_use]
pub fn describe(path: &Path, message: &str) -> Option<String> {
    (cfg!(target_os = "macos") && is_blocked(message))
        .then(|| format!("{MARKER}{}", path.display()))
}

/// `message`, or the folder-access marker when macOS blocked `path`.
#[must_use]
pub fn tag(path: &Path, message: String) -> String {
    describe(path, &message).unwrap_or(message)
}

/// Opens System Settings at Files and Folders, where the user allows the folder.
#[tauri::command]
pub async fn open_folder_access_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = tauri::async_runtime::spawn_blocking(|| {
            std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_FilesAndFolders")
                .status()
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Could not open System Settings.".to_owned())
        }
    }
    #[cfg(not(target_os = "macos"))]
    Err("Folder access settings exist only on macOS.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::is_blocked;

    #[test]
    fn only_the_not_permitted_failure_counts_as_a_blocked_folder() {
        assert!(is_blocked("Operation not permitted (os error 1)"));
        assert!(is_blocked(
            "fatal: cannot open '/Users/a/Documents/notes': Operation not permitted"
        ));
        assert!(is_blocked("could not read the folder (os error 1)"));
        assert!(!is_blocked("Permission denied (os error 13)"));
        assert!(!is_blocked("No such file or directory (os error 2)"));
    }
}
