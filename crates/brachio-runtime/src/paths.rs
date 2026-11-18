//! Where the runtime and the app keep their files for one user.

use std::env;
use std::io;
use std::path::PathBuf;

/// The folder for runtime state: saved screens, and the copy of the executable that runs.
///
/// # Errors
/// Fails when the user's home or data folder is not set in the environment.
pub fn state_directory() -> io::Result<PathBuf> {
    let missing =
        |name: &str| io::Error::new(io::ErrorKind::NotFound, format!("{name} is not set"));
    #[cfg(windows)]
    let base = PathBuf::from(env::var_os("LOCALAPPDATA").ok_or_else(|| missing("LOCALAPPDATA"))?)
        .join("Brachio");
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(env::var_os("HOME").ok_or_else(|| missing("HOME"))?)
        .join("Library/Application Support/Brachio");
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = match env::var_os("XDG_STATE_HOME") {
        Some(state) => PathBuf::from(state),
        None => {
            PathBuf::from(env::var_os("HOME").ok_or_else(|| missing("HOME"))?).join(".local/state")
        }
    }
    .join("brachio");
    Ok(base.join("runtime"))
}
