//! Where the runtime and the app keep their files for one user.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

/// Creates `directory`, and the state folder above it, readable by this user alone where the
/// platform has file modes. On Windows the folder sits under the user's profile, which other
/// accounts cannot read.
///
/// # Errors
/// Fails when the folder cannot be created or its mode cannot be set.
pub fn ensure_private_directory(directory: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
        let state = state_directory()?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        if directory.starts_with(&state) {
            fs::set_permissions(&state, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(directory)
    }
}
