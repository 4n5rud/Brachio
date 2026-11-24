//! An app opened from Finder or the Dock gets launchd's minimal `PATH`, so Homebrew and
//! `~/.local/bin` tools, and the agents installed there, are not found. Read the `PATH` a login
//! shell would have and merge it into the app's own.

use std::env;
use std::process::Command;
use std::time::Duration;

const MARKER: &str = "__BRACHIO_PATH__";
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(3);

/// Adds the login shell's `PATH` entries to this process's `PATH` on macOS.
pub fn import_login_path() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
    let mut command = Command::new(&shell);
    command
        .arg("-l")
        .arg("-c")
        .arg(format!("printf '{MARKER}%s{MARKER}' \"$PATH\""));
    let Ok(output) = brachio_process::output_within(&mut command, LOGIN_SHELL_TIMEOUT) else {
        tracing::warn!("could not read the login shell's PATH");
        return;
    };
    let printed = String::from_utf8_lossy(&output.stdout);
    let Some(login) = extract_path(&printed) else {
        return;
    };
    let current = env::var("PATH").unwrap_or_default();
    env::set_var("PATH", merge_paths(&login, &current));
}

/// The text between the two markers, which survives whatever the profile files print.
fn extract_path(output: &str) -> Option<String> {
    let start = output.find(MARKER)? + MARKER.len();
    let end = output[start..].find(MARKER)? + start;
    let path = &output[start..end];
    (!path.is_empty()).then(|| path.to_owned())
}

/// Entries of `login` first, then the entries of `current` that it lacks.
fn merge_paths(login: &str, current: &str) -> String {
    let mut merged: Vec<&str> = Vec::new();
    for entry in login.split(':').chain(current.split(':')) {
        if !entry.is_empty() && !merged.contains(&entry) {
            merged.push(entry);
        }
    }
    merged.join(":")
}

#[cfg(test)]
mod tests {
    use super::{extract_path, merge_paths};

    #[test]
    fn the_path_is_read_past_profile_noise() {
        let printed = "Welcome\n__BRACHIO_PATH__/opt/homebrew/bin:/usr/bin__BRACHIO_PATH__";
        assert_eq!(
            extract_path(printed).as_deref(),
            Some("/opt/homebrew/bin:/usr/bin")
        );
        assert_eq!(extract_path("no markers"), None);
        assert_eq!(extract_path("__BRACHIO_PATH____BRACHIO_PATH__"), None);
    }

    #[test]
    fn login_entries_come_first_without_duplicates() {
        assert_eq!(
            merge_paths("/opt/homebrew/bin:/usr/bin", "/usr/bin:/bin:/custom"),
            "/opt/homebrew/bin:/usr/bin:/bin:/custom"
        );
    }
}
