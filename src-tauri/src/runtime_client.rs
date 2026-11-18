//! Connects the app to the independent terminal runtime, starting it when it is not running.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use brachio_runtime::client::{Client, ClientError};
use brachio_runtime::wire::Action;

/// `1` forces terminals into the runtime, `0` keeps them in the app. Unset, the runtime is used
/// whenever its executable can be found.
pub const OPT_IN_VARIABLE: &str = "BRACHIO_TERMINAL_RUNTIME";
/// Overrides the runtime endpoint name, for side-by-side installs and tests.
const ENDPOINT_VARIABLE: &str = "BRACHIO_RUNTIME_ENDPOINT";
/// Overrides where the runtime executable is looked up.
const EXECUTABLE_VARIABLE: &str = "BRACHIO_RUNTIME_EXECUTABLE";
const EXE_SUFFIX: &str = if cfg!(windows) { ".exe" } else { "" };
const STARTUP_ATTEMPTS: u32 = 75;
const STARTUP_STEP: Duration = Duration::from_millis(40);

/// Whether terminals should live in the runtime.
pub fn enabled() -> bool {
    match env::var(OPT_IN_VARIABLE).as_deref() {
        Ok("0") => false,
        Ok("1") => true,
        _ => executable().is_ok(),
    }
}

/// The endpoint of this user's runtime. Only letters, digits and `-` survive, so it is a valid
/// pipe name and a valid file name on every platform.
pub fn endpoint() -> String {
    if let Ok(name) = env::var(ENDPOINT_VARIABLE) {
        return name;
    }
    let user = env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_default();
    let clean: String = user
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(16)
        .collect();
    format!("brachio-{clean}")
}

/// A client for this user's runtime, after making sure one is running.
///
/// # Errors
/// Fails when no runtime answers and none could be started.
pub async fn connect() -> Result<Client> {
    let name = endpoint();
    let client = Client::new(&name);
    match client.call(Action::Hello).await {
        Ok(_) => return Ok(client),
        Err(ClientError::NotRunning) => {}
        Err(error) => bail!("{error}"),
    }
    start(&name)?;
    for _ in 0..STARTUP_ATTEMPTS {
        if client.call(Action::Hello).await.is_ok() {
            return Ok(client);
        }
        tokio::time::sleep(STARTUP_STEP).await;
    }
    bail!("the terminal runtime did not start in time")
}

fn start(endpoint: &str) -> Result<()> {
    let executable = stable_copy(&executable()?)?;
    let mut command = Command::new(&executable);
    command
        .args(["--pipe", endpoint, "serve"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command);
    command
        .spawn()
        .with_context(|| format!("start {}", executable.display()))?;
    Ok(())
}

/// The runtime executable: the override, else the one installed next to the app.
fn executable() -> Result<PathBuf> {
    if let Some(path) = env::var_os(EXECUTABLE_VARIABLE) {
        return Ok(PathBuf::from(path));
    }
    let name = if cfg!(windows) {
        "brachio-runtime.exe"
    } else {
        "brachio-runtime"
    };
    let beside = env::current_exe()?
        .parent()
        .map(|folder| folder.join(name))
        .filter(|path| path.exists());
    beside.with_context(|| format!("{name} is not installed next to the app"))
}

/// A private copy of the runtime to run. The build output stays free to be replaced while a
/// runtime is alive, which Windows would otherwise refuse, and each build gets its own copy.
fn stable_copy(source: &Path) -> Result<PathBuf> {
    let folder = brachio_runtime::paths::state_directory()?.join("bin");
    fs::create_dir_all(&folder)?;
    let metadata = fs::metadata(source).with_context(|| format!("read {}", source.display()))?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let name = format!(
        "brachio-runtime-{}-{modified}{}",
        metadata.len(),
        EXE_SUFFIX
    );
    let copy = folder.join(&name);
    if !copy.exists() {
        let scratch = folder.join(format!("{name}.part"));
        fs::copy(source, &scratch)?;
        fs::rename(&scratch, &copy)?;
    }
    remove_older_copies(&folder, &name);
    Ok(copy)
}

/// Copies still in use cannot be removed on Windows; they go on a later start.
fn remove_older_copies(folder: &Path, keep: &str) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with("brachio-runtime-") && file_name != keep {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Keep the runtime alive when the app exits: no console, its own process group.
#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(test)]
mod tests {
    use super::endpoint;

    #[test]
    fn the_endpoint_is_a_plain_name() {
        let name = endpoint();
        assert!(name.starts_with("brachio-"));
        assert!(name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-'));
    }
}
