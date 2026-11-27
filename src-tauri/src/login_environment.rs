//! An app opened from Finder or the Dock gets launchd's minimal `PATH`, so Homebrew and
//! `~/.local/bin` tools, and the agents installed there, are not found. Read the `PATH` the
//! user's shell would have and merge it into the app's own.
//!
//! The shell runs as an interactive login shell, because installers such as Claude Code's add
//! `~/.local/bin` to `~/.zshrc`, which a non-interactive login shell does not read. If that
//! shell fails (a `.zshrc` that hangs or `exec`s another program), a plain login shell is tried.
//! The usual per-user install folders are appended last, so an agent installed there is found
//! even when no shell reports its folder.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const MARKER: &str = "__BRACHIO_PATH__";
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(3);

/// Shell flags to try in order: interactive login first, then login only.
const SHELL_FLAG_SETS: [&[&str]; 2] = [&["-i", "-l", "-c"], &["-l", "-c"]];

/// Adds the user's shell `PATH` entries, then the usual install folders, to this process's
/// `PATH` on macOS.
pub fn import_login_path() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
    let login = SHELL_FLAG_SETS
        .iter()
        .find_map(|flags| read_path(shell_command(&shell, flags)));
    if login.is_none() {
        tracing::warn!("could not read the login shell's PATH");
    }
    let current = env::var("PATH").unwrap_or_default();
    let installs = env::var_os("HOME")
        .map(|home| install_folders(Path::new(&home)))
        .unwrap_or_default()
        .into_iter()
        .filter(|folder| folder.is_dir())
        .filter_map(|folder| folder.to_str().map(str::to_owned))
        .collect::<Vec<_>>()
        .join(":");
    env::set_var(
        "PATH",
        merge_paths(&[login.as_deref().unwrap_or_default(), &current, &installs]),
    );
}

/// `shell` started with `flags`, set to print its `PATH` between markers.
fn shell_command(shell: &str, flags: &[&str]) -> Command {
    let mut command = Command::new(shell);
    command
        .args(flags)
        .arg(format!("printf '{MARKER}%s{MARKER}' \"$PATH\""));
    command
}

/// The `PATH` that `command` prints, or `None` if it fails or times out.
fn read_path(mut command: Command) -> Option<String> {
    let output = brachio_process::output_within(&mut command, LOGIN_SHELL_TIMEOUT).ok()?;
    extract_path(&String::from_utf8_lossy(&output.stdout))
}

/// Folders where agents and developer tools are commonly installed for one user.
fn install_folders(home: &Path) -> Vec<PathBuf> {
    vec![
        // Claude Code's native installer, pipx and many other per-user installers.
        home.join(".local/bin"),
        // Claude Code's older local install.
        home.join(".claude/local"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]
}

/// The text between the two markers, which survives whatever the profile files print.
fn extract_path(output: &str) -> Option<String> {
    let start = output.find(MARKER)? + MARKER.len();
    let end = output[start..].find(MARKER)? + start;
    let path = &output[start..end];
    (!path.is_empty()).then(|| path.to_owned())
}

/// The entries of each `PATH` in order, skipping empty entries and ones already taken.
fn merge_paths(paths: &[&str]) -> String {
    let mut merged: Vec<&str> = Vec::new();
    for entry in paths.iter().flat_map(|path| path.split(':')) {
        if !entry.is_empty() && !merged.contains(&entry) {
            merged.push(entry);
        }
    }
    merged.join(":")
}

#[cfg(test)]
mod tests {
    use super::{extract_path, install_folders, merge_paths, read_path, shell_command};
    use std::path::Path;

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
            merge_paths(&["/opt/homebrew/bin:/usr/bin", "/usr/bin:/bin:/custom"]),
            "/opt/homebrew/bin:/usr/bin:/bin:/custom"
        );
    }

    #[test]
    fn install_folders_come_after_the_shell_and_app_paths() {
        assert_eq!(
            merge_paths(&[
                "/opt/homebrew/bin",
                "/usr/bin:/bin",
                "/fake-home/.local/bin:/usr/bin"
            ]),
            "/opt/homebrew/bin:/usr/bin:/bin:/fake-home/.local/bin"
        );
        assert_eq!(merge_paths(&["", "/usr/bin", ""]), "/usr/bin");
    }

    #[test]
    fn the_per_user_install_folder_is_listed_first() {
        let folders = install_folders(Path::new("/fake-home"));
        assert_eq!(folders[0], Path::new("/fake-home/.local/bin"));
        assert!(folders.contains(&Path::new("/fake-home/.claude/local").to_path_buf()));
    }

    #[cfg(unix)]
    #[test]
    fn an_interactive_shell_reads_its_rc_file() {
        // `sh` reads `$ENV` only when interactive, like zsh reads `.zshrc`.
        let home = tempfile::tempdir().expect("temporary folder");
        let rc = home.path().join("rc");
        std::fs::write(&rc, "PATH=/from/rc:$PATH\n").expect("write rc file");
        let read = |flags: &[&str]| {
            let mut command = shell_command("/bin/sh", flags);
            command.env("ENV", &rc);
            read_path(command).expect("shell PATH")
        };
        let interactive = read(&["-i", "-c"]);
        let plain = read(&["-c"]);
        assert!(interactive.starts_with("/from/rc:"), "{interactive}");
        assert!(!plain.contains("/from/rc"), "{plain}");
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_cannot_start_gives_no_path() {
        assert_eq!(
            read_path(shell_command("/nonexistent/shell", &["-c"])),
            None
        );
    }
}
