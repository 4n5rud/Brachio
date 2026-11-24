//! The `PATH` of the user's login shell, for an app opened from Finder or the Dock.
//!
//! macOS starts such an app with the minimal `PATH` of `launchd` (`/usr/bin:/bin:/usr/sbin:/sbin`),
//! so Homebrew, npm and the like are not on it and agents installed there are reported as missing.
//! A terminal gets them because the shell's startup files add them. This asks the user's login shell
//! for its `PATH` once at start and puts the missing folders after the ones the app already has.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Printed around the value so anything a startup file prints is ignored.
const MARK: &str = "__BRACHIO_PATH__";

/// How long a login shell may take to answer; slow startup files must not hold the app back.
pub const SHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// The `PATH` between the two marks in `output`, when it looks like a `PATH`.
fn parse_marked_path(output: &str) -> Option<String> {
    let start = output.find(MARK)? + MARK.len();
    let end = start + output[start..].find(MARK)?;
    let path = &output[start..end];
    (!path.is_empty()
        && !path.contains(['\0', '\n', '\r'])
        && path.split(':').all(|folder| folder.starts_with('/')))
    .then(|| path.to_owned())
}

/// `current` followed by the folders of `login` that it does not hold yet. The app's own folders
/// keep their place and priority; nothing is removed.
#[must_use]
pub fn merge_paths(current: &str, login: &str) -> String {
    let mut folders: Vec<&str> = current
        .split(':')
        .filter(|folder| !folder.is_empty())
        .collect();
    for folder in login.split(':').filter(|folder| !folder.is_empty()) {
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    folders.join(":")
}

/// Asks `shell`, started as a login shell, for its `PATH`. `None` when it fails, answers nonsense
/// or takes longer than `timeout`; the shell is then stopped.
#[must_use]
pub fn path_from_login_shell(shell: &Path, timeout: Duration) -> Option<String> {
    let script = format!("printf '%s%s%s' '{MARK}' \"$PATH\" '{MARK}'");
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.by_ref().take(64 * 1024).read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });
    if let Ok(bytes) = receiver.recv_timeout(timeout) {
        let _ = child.wait();
        parse_marked_path(&String::from_utf8_lossy(&bytes))
    } else {
        let _ = child.kill();
        let _ = child.wait();
        None
    }
}

/// On macOS, adds the login shell's folders to this process's `PATH`, before any thread or child
/// program reads it. Other systems already start apps with the user's `PATH`.
pub fn import_login_path() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let shell = std::env::var_os("SHELL").map_or_else(
        || std::path::PathBuf::from("/bin/zsh"),
        std::path::PathBuf::from,
    );
    let current = std::env::var("PATH").unwrap_or_default();
    match path_from_login_shell(&shell, SHELL_TIMEOUT) {
        Some(login) => std::env::set_var("PATH", merge_paths(&current, &login)),
        None => tracing::warn!(
            "could not read the login shell's PATH; agents are found only on the app's own PATH"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_paths, parse_marked_path, path_from_login_shell, MARK};
    use std::time::Duration;

    #[test]
    fn the_path_is_read_from_between_the_marks_and_noise_is_ignored() {
        let output =
            format!("Last login: today\nnvm loaded\n{MARK}/opt/homebrew/bin:/usr/bin{MARK}\nbye");
        assert_eq!(
            parse_marked_path(&output).as_deref(),
            Some("/opt/homebrew/bin:/usr/bin")
        );
    }

    #[test]
    fn nonsense_between_the_marks_is_refused() {
        for output in [
            String::new(),
            format!("{MARK}{MARK}"),
            format!("{MARK}/usr/bin"),
            format!("{MARK}relative/bin:/usr/bin{MARK}"),
            format!("{MARK}/usr/bin\n/etc{MARK}"),
            format!("{MARK}/usr/bin:{MARK}"),
        ] {
            assert_eq!(parse_marked_path(&output), None, "{output:?}");
        }
    }

    #[test]
    fn the_apps_own_folders_keep_their_place_and_new_ones_follow_without_repeats() {
        assert_eq!(
            merge_paths(
                "/usr/bin:/bin",
                "/opt/homebrew/bin:/usr/bin:/Users/me/.cargo/bin:/opt/homebrew/bin"
            ),
            "/usr/bin:/bin:/opt/homebrew/bin:/Users/me/.cargo/bin"
        );
        assert_eq!(merge_paths("", "/a:/b"), "/a:/b");
        assert_eq!(merge_paths("/a::/b", ""), "/a:/b");
    }

    #[cfg(unix)]
    fn script(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().expect("folder");
        let path = folder.path().join("fake-shell");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
        (folder, path)
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_prints_a_path_after_some_noise_is_understood() {
        let (_folder, shell) = script(&format!(
            "echo noise\nprintf '%s' '{MARK}/opt/x/bin:/usr/bin{MARK}'"
        ));
        assert_eq!(
            path_from_login_shell(&shell, Duration::from_secs(5)).as_deref(),
            Some("/opt/x/bin:/usr/bin")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_takes_too_long_is_stopped_and_gives_nothing() {
        let (_folder, shell) = script("sleep 30");
        let started = std::time::Instant::now();
        assert_eq!(
            path_from_login_shell(&shell, Duration::from_millis(300)),
            None
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "it must not wait for the shell"
        );
    }

    #[test]
    fn a_missing_shell_gives_nothing() {
        assert_eq!(
            path_from_login_shell(
                std::path::Path::new("/nonexistent/shell"),
                Duration::from_millis(300)
            ),
            None
        );
    }
}
