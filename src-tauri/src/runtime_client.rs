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
/// pipe name and a valid file name on every platform. A hash of the full user name and home folder
/// keeps accounts apart whose names look alike once cut down, such as `john.doe` and `johndoe`,
/// or are not ASCII at all.
pub fn endpoint() -> String {
    if let Ok(name) = env::var(ENDPOINT_VARIABLE) {
        return name;
    }
    generation_endpoint(&current_user(), &home_folder(), GENERATION)
}

/// Raised when a change to the runtime has to reach users who update while an older runtime still
/// hosts their agents. Each generation has an endpoint of its own, so the new runtime starts
/// beside the old one, which keeps serving the agents it has until they end.
pub const GENERATION: u32 = 3;

/// The endpoint of one generation. The first generation has no suffix, which is the name earlier
/// versions used.
fn generation_endpoint(user: &str, home: &str, generation: u32) -> String {
    let base = endpoint_for(user, home);
    if generation <= 1 {
        base
    } else {
        format!("{base}-g{generation}")
    }
}

/// The socket agents reach the app on for approvals and messages. It does not change with the
/// generation: an agent keeps the name it was started with for as long as it lives, and an agent
/// in an older runtime must still reach the app.
pub fn hooks_endpoint() -> String {
    let base = env::var(ENDPOINT_VARIABLE)
        .unwrap_or_else(|_| generation_endpoint(&current_user(), &home_folder(), 1));
    format!("{base}-hooks")
}

/// Endpoints of older generations, newest first, then the name used before generations.
fn older_endpoints() -> Vec<String> {
    if env::var(ENDPOINT_VARIABLE).is_ok() {
        return Vec::new();
    }
    let (user, home) = (current_user(), home_folder());
    let mut names: Vec<String> = (1..GENERATION)
        .rev()
        .map(|generation| generation_endpoint(&user, &home, generation))
        .collect();
    names.extend(legacy_endpoint());
    names
}

/// Candidate endpoints of older runtimes, whether they are running or not.
pub fn older_clients() -> impl Iterator<Item = Client> {
    older_endpoints().into_iter().map(|name| Client::new(&name))
}

fn current_user() -> String {
    env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_default()
}

fn home_folder() -> String {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_default()
}

fn endpoint_for(user: &str, home: &str) -> String {
    let clean = readable_part(user);
    let hash = stable_hash(&format!("{user}\0{home}"));
    if clean.is_empty() {
        format!("brachio-{hash:08x}")
    } else {
        format!("brachio-{clean}-{hash:08x}")
    }
}

fn readable_part(user: &str) -> String {
    user.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(16)
        .collect()
}

/// FNV-1a, folded to 32 bits. Unlike the standard hasher it gives the same answer in every build,
/// which matters because a new build must find the runtime an old build started.
fn stable_hash(text: &str) -> u32 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    u32::try_from((hash >> 32) ^ (hash & 0xffff_ffff)).unwrap_or(0)
}

/// The name earlier versions used, without the hash. A runtime still running under it is kept
/// until it ends, so sessions started before an update are not lost from view.
fn legacy_endpoint() -> Option<String> {
    if env::var(ENDPOINT_VARIABLE).is_ok() {
        return None;
    }
    let clean = readable_part(&current_user());
    (!clean.is_empty()).then(|| format!("brachio-{clean}"))
}

/// A client for this user's runtime, after making sure one is running.
///
/// # Errors
/// Fails when no runtime answers and none could be started.
pub async fn connect() -> Result<Client> {
    let client = connect_current().await?;
    retire_older().await;
    Ok(client)
}

async fn connect_current() -> Result<Client> {
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

/// Tells older runtimes to take no new terminals and to end once their agents have. Only done
/// after the current runtime answered, so a failed start never leaves nothing to open terminals
/// in. A runtime too old to know the request just ignores it.
async fn retire_older() {
    for old in older_clients() {
        let _ = old.call(Action::Drain).await;
    }
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
    copy_conpty_files(source, &folder);
    Ok(copy)
}

/// The runtime loads the newer ConPTY from beside its executable, so the copy needs the files
/// too. A file in use or a failed copy only means the system ConPTY is used.
fn copy_conpty_files(source: &Path, folder: &Path) {
    let Some(conpty) = source.parent().map(|parent| parent.join("conpty")) else {
        return;
    };
    for name in ["conpty.dll", "OpenConsole.exe"] {
        let target = folder.join(name);
        if conpty.join(name).is_file() && !target.exists() {
            let _ = fs::copy(conpty.join(name), target);
        }
    }
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

#[cfg(test)]
mod endpoint_tests {
    use super::endpoint_for;

    #[test]
    fn accounts_with_alike_or_non_ascii_names_get_different_endpoints() {
        let a = endpoint_for("john.doe", r"C:\Users\john.doe");
        let b = endpoint_for("johndoe", r"C:\Users\johndoe");
        assert_ne!(a, b);
        assert_ne!(
            endpoint_for("홍길동", r"C:\Users\홍길동"),
            endpoint_for("김철수", r"C:\Users\김철수")
        );
        assert_ne!(
            endpoint_for("averyveryverylongusername1", "/home/a"),
            endpoint_for("averyveryverylongusername2", "/home/b")
        );
    }

    #[test]
    fn each_generation_has_its_own_endpoint_and_the_first_keeps_the_old_name() {
        use super::generation_endpoint;
        let first = generation_endpoint("me", "/home/me", 1);
        assert_eq!(first, endpoint_for("me", "/home/me"));
        let second = generation_endpoint("me", "/home/me", 2);
        assert_ne!(first, second);
        assert!(second.starts_with(&first) && second.ends_with("-g2"));
        assert_ne!(second, generation_endpoint("me", "/home/me", 3));
    }

    #[test]
    fn an_endpoint_is_stable_and_only_letters_digits_and_hyphens() {
        let name = endpoint_for("홍길동", r"C:\Users\홍길동");
        assert_eq!(name, endpoint_for("홍길동", r"C:\Users\홍길동"));
        assert!(name.starts_with("brachio-"));
        assert!(name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-'));
        assert_eq!(
            endpoint_for("4n5rud", "x"),
            "brachio-4n5rud-".to_owned() + &name_hash("4n5rud", "x")
        );
    }

    fn name_hash(user: &str, home: &str) -> String {
        endpoint_for(user, home)
            .rsplit('-')
            .next()
            .expect("hash part")
            .to_owned()
    }
}
