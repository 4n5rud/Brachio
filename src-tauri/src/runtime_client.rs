//! Connects the app to the independent terminal runtime, starting it when it is not running.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use brachio_runtime::client::{Client, ClientError};
use brachio_runtime::wire::Action;
use sha2::{Digest, Sha256};

/// `1` forces terminals into the runtime, `0` keeps them in the app. Unset, the runtime is used
/// whenever its executable can be found.
pub const OPT_IN_VARIABLE: &str = "BRACHIO_TERMINAL_RUNTIME";
/// Overrides the runtime endpoint name, for side-by-side installs and tests.
const ENDPOINT_VARIABLE: &str = "BRACHIO_RUNTIME_ENDPOINT";
/// Overrides where the runtime executable is looked up.
const EXECUTABLE_VARIABLE: &str = "BRACHIO_RUNTIME_EXECUTABLE";
const RUNTIME_NAME: &str = if cfg!(windows) {
    "brachio-runtime.exe"
} else {
    "brachio-runtime"
};
const CONPTY_FILES: [&str; 2] = ["conpty.dll", "OpenConsole.exe"];
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
pub const GENERATION: u32 = 5;

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

/// Endpoints this installation's runtimes may serve, current first, then the older generations.
pub fn own_endpoints() -> Vec<String> {
    let mut names = vec![endpoint()];
    names.extend(older_endpoints());
    names
}

/// Clients of the older runtimes that are still running.
pub async fn older_clients() -> Vec<Client> {
    let mut running = Vec::new();
    for name in older_endpoints() {
        let client = Client::new(&name);
        if client.call(Action::Hello).await.is_ok() {
            running.push(client);
        }
    }
    running
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
    for old in older_clients().await {
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
    let beside = env::current_exe()?
        .parent()
        .map(|folder| folder.join(RUNTIME_NAME))
        .filter(|path| path.exists());
    beside.with_context(|| format!("{RUNTIME_NAME} is not installed next to the app"))
}

/// A private copy of the runtime to run. The build output stays free to be replaced while a
/// runtime is alive, which Windows would otherwise refuse, and each build gets its own copy.
fn stable_copy(source: &Path) -> Result<PathBuf> {
    let folder = brachio_runtime::paths::state_directory()?.join("bin");
    stage_runtime(source, &folder)
}

/// Publish the executable and its optional ConPTY pair as one immutable directory. Keep older
/// bundles because a draining runtime can still need its helpers while serving its terminals.
fn stage_runtime(source: &Path, folder: &Path) -> Result<PathBuf> {
    let mut files = vec![(RUNTIME_NAME, source.to_owned())];
    if let Some(parent) = source.parent() {
        let conpty = parent.join("conpty");
        for name in CONPTY_FILES {
            let path = conpty.join(name);
            if path.try_exists()? {
                files.push((name, path));
            }
        }
    }
    anyhow::ensure!(
        files.len() != 2,
        "the bundled ConPTY helper pair is incomplete"
    );

    let mut bundle_digest = Sha256::new();
    let files = files
        .into_iter()
        .map(|(name, path)| {
            let digest = file_digest(&path)?;
            bundle_digest.update(digest.as_bytes());
            Ok((name, path, digest))
        })
        .collect::<Result<Vec<_>>>()?;

    let destination = folder.join(format!("brachio-runtime-{:x}", bundle_digest.finalize()));
    let complete = || {
        files
            .iter()
            .all(|(name, _, _)| destination.join(name).is_file())
    };
    if !complete() {
        fs::create_dir_all(folder)?;
        let scratch = folder.join(format!(".{}.part", uuid::Uuid::new_v4()));
        fs::create_dir(&scratch)?;
        let result = (|| -> Result<()> {
            for (name, source, digest) in &files {
                let target = scratch.join(name);
                fs::copy(source, &target)?;
                anyhow::ensure!(
                    file_digest(&target)? == *digest,
                    "{name} changed while staging"
                );
            }
            match fs::rename(&scratch, &destination) {
                Ok(()) => Ok(()),
                Err(_) if complete() => Ok(()),
                Err(error) => Err(error.into()),
            }
        })();
        for (name, _, _) in &files {
            let _ = fs::remove_file(scratch.join(name));
        }
        let _ = fs::remove_dir(scratch);
        result?;
    }
    Ok(destination.join(RUNTIME_NAME))
}

fn file_digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| format!("read {}", path.display()))?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
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
    use anyhow::Context;

    use super::endpoint;

    #[test]
    fn conpty_upgrade_keeps_each_running_version_with_its_own_helpers() -> anyhow::Result<()> {
        use std::fs;

        let dir = tempfile::tempdir()?;
        let source = dir.path().join("runtime.exe");
        let cache = dir.path().join("bin");
        let bundled = dir.path().join("conpty");
        fs::create_dir(&bundled)?;
        fs::write(&source, b"same runtime")?;
        fs::write(bundled.join("conpty.dll"), b"old dll")?;
        fs::write(bundled.join("OpenConsole.exe"), b"old console")?;
        let old = super::stage_runtime(&source, &cache)?;

        fs::write(bundled.join("conpty.dll"), b"new dll")?;
        fs::write(bundled.join("OpenConsole.exe"), b"new console")?;
        let new = super::stage_runtime(&source, &cache)?;

        let old_dir = old.parent().context("old generation directory")?;
        let new_dir = new.parent().context("new generation directory")?;
        assert_ne!(old_dir, new_dir);
        assert_eq!(fs::read(old_dir.join("conpty.dll"))?, b"old dll");
        assert_eq!(fs::read(old_dir.join("OpenConsole.exe"))?, b"old console");
        assert_eq!(fs::read(new_dir.join("conpty.dll"))?, b"new dll");
        assert_eq!(fs::read(new_dir.join("OpenConsole.exe"))?, b"new console");
        assert_eq!(super::stage_runtime(&source, &cache)?, new);
        Ok(())
    }

    #[test]
    fn swapped_executables_with_matching_metadata_get_different_cache_entries() -> anyhow::Result<()>
    {
        use std::fs::{self, File, FileTimes};
        use std::time::{Duration, UNIX_EPOCH};

        let dir = tempfile::tempdir()?;
        let source = dir.path().join("runtime.exe");
        let cache = dir.path().join("bin");
        let modified = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        fs::write(&source, b"first")?;
        File::options()
            .write(true)
            .open(&source)?
            .set_times(FileTimes::new().set_modified(modified))?;
        let first = super::stage_runtime(&source, &cache)?;
        fs::write(&source, b"other")?;
        File::options()
            .write(true)
            .open(&source)?
            .set_times(FileTimes::new().set_modified(modified))?;
        let second = super::stage_runtime(&source, &cache)?;
        assert_ne!(first, second);
        assert_eq!(fs::read(&second)?, b"other");
        assert_eq!(super::stage_runtime(&source, &cache)?, second);
        Ok(())
    }

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

/// The runtime is chosen through process-wide environment variables, so tests that read or set
/// them run one at a time.
#[cfg(test)]
pub(crate) static TEST_ENVIRONMENT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Keeps terminals in the app for a test that does not use the runtime, so it never meets another
/// test's runtime settings or the real runtime of the machine.
#[cfg(test)]
pub(crate) struct RuntimeOff {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    previous: Option<std::ffi::OsString>,
}

#[cfg(test)]
impl RuntimeOff {
    pub(crate) fn hold() -> Self {
        let lock = TEST_ENVIRONMENT.blocking_lock();
        let previous = env::var_os(OPT_IN_VARIABLE);
        env::set_var(OPT_IN_VARIABLE, "0");
        Self {
            _lock: lock,
            previous,
        }
    }
}

#[cfg(test)]
impl Drop for RuntimeOff {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => env::set_var(OPT_IN_VARIABLE, value),
            None => env::remove_var(OPT_IN_VARIABLE),
        }
    }
}
