//! The shells installed on this machine: what the "+" menu offers and what Settings picks from.

use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// An installed shell and how to start it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shell {
    /// Stable name used by tabs and settings, such as `pwsh`, `wsl:Ubuntu` or `zsh`.
    pub id: String,
    /// What the menu shows.
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// A shell as the window sees it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellInfo {
    id: String,
    name: String,
    is_default: bool,
}

#[derive(Default)]
struct Detected {
    shells: Option<Arc<Vec<Shell>>>,
    default_id: Option<String>,
}

static STATE: Mutex<Detected> = Mutex::new(Detected {
    shells: None,
    default_id: None,
});

fn state() -> std::sync::MutexGuard<'static, Detected> {
    STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The detected shells. The first call looks for them; later calls reuse the answer.
pub fn installed() -> Arc<Vec<Shell>> {
    if let Some(found) = state().shells.clone() {
        return found;
    }
    // Detection runs programs, so it must not hold the lock.
    let found = Arc::new(detect());
    state().shells.get_or_insert(found).clone()
}

/// Looks a shell up by id.
pub fn find(id: &str) -> Option<Shell> {
    installed().iter().find(|shell| shell.id == id).cloned()
}

/// The shell chosen in Settings, when it is still installed.
pub fn chosen_default() -> Option<Shell> {
    let id = state().default_id.clone()?;
    find(&id)
}

/// Lists the shells; `refresh` looks again, for a shell installed while the app runs.
#[tauri::command]
pub async fn shell_list(refresh: bool) -> Result<Vec<ShellInfo>, String> {
    tokio::task::spawn_blocking(move || {
        if refresh {
            state().shells = None;
        }
        let default_id = chosen_default().map(|shell| shell.id);
        installed()
            .iter()
            .enumerate()
            .map(|(index, shell)| ShellInfo {
                id: shell.id.clone(),
                name: shell.name.clone(),
                is_default: default_id
                    .as_deref()
                    .map_or(index == 0, |chosen| chosen == shell.id),
            })
            .collect()
    })
    .await
    .map_err(|error| error.to_string())
}

/// Remembers which shell plain shell tabs start; `None` goes back to the system's own.
#[tauri::command]
pub fn shell_set_default(id: Option<String>) {
    state().default_id = id;
}

#[cfg(windows)]
fn detect() -> Vec<Shell> {
    let mut found = Vec::new();
    let system_root =
        env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let system32 = system_root.join("System32");
    let on_path = |program: &str| {
        brachio_agent::find_on_path(
            program,
            env::var_os("PATH").as_deref(),
            env::var_os("PATHEXT").as_deref(),
        )
    };
    let program_files = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(env::var_os)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let pwsh = on_path("pwsh").or_else(|| {
        program_files
            .iter()
            .map(|root| root.join("PowerShell").join("7").join("pwsh.exe"))
            .find(|candidate| candidate.is_file())
    });
    if let Some(program) = pwsh {
        found.push(plain("pwsh", "PowerShell 7", program));
    }
    let legacy = system32
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    if legacy.is_file() {
        found.push(plain("powershell", "Windows PowerShell", legacy));
    }
    let cmd = env::var_os("COMSPEC")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .unwrap_or_else(|| system32.join("cmd.exe"));
    if cmd.is_file() {
        found.push(plain("cmd", "Command Prompt", cmd));
    }
    let git_roots = program_files.iter().map(|root| root.join("Git")).chain(
        env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join("Programs").join("Git")),
    );
    for root in git_roots {
        let bash = root.join("bin").join("bash.exe");
        if bash.is_file() {
            found.push(Shell {
                args: vec!["--login".to_owned(), "-i".to_owned()],
                ..plain("git-bash", "Git Bash", bash)
            });
            break;
        }
    }
    let wsl = system32.join("wsl.exe");
    if wsl.is_file() {
        for distro in wsl_distributions(&wsl) {
            found.push(Shell {
                id: format!("wsl:{distro}"),
                name: format!("{distro} (WSL)"),
                program: wsl.clone(),
                args: vec!["-d".to_owned(), distro],
            });
        }
    }
    dedupe(found)
}

#[cfg(windows)]
fn plain(id: &str, name: &str, program: PathBuf) -> Shell {
    Shell {
        id: id.to_owned(),
        name: name.to_owned(),
        program,
        args: Vec::new(),
    }
}

#[cfg(windows)]
fn wsl_distributions(wsl: &Path) -> Vec<String> {
    let mut command = std::process::Command::new(wsl);
    command.args(["-l", "-q"]);
    // A machine without WSL set up answers with an error or hangs on a prompt.
    brachio_process::output_within(&mut command, std::time::Duration::from_secs(3))
        .ok()
        .filter(|output| output.status.success())
        .map(|output| decode_wsl_list(&output.stdout))
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn detect() -> Vec<Shell> {
    let login = env::var_os("SHELL").map(PathBuf::from);
    let listed = std::fs::read_to_string("/etc/shells").unwrap_or_default();
    unix_shells(login.as_deref(), &listed, Path::is_file)
}

/// `wsl -l -q` prints UTF-16 with a byte order mark, one distribution per line. Docker's
/// internal distributions are not a shell anyone opens.
#[cfg(any(windows, test))]
fn decode_wsl_list(bytes: &[u8]) -> Vec<String> {
    let utf16 = bytes.starts_with(&[0xff, 0xfe])
        || (bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).all(|byte| *byte == 0));
    let text = if utf16 {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.trim_start_matches('\u{feff}')
        .lines()
        .map(|line| line.trim().trim_matches('\u{0}'))
        .filter(|name| !name.is_empty() && !name.starts_with("docker-desktop"))
        .map(str::to_owned)
        .collect()
}

/// The login shell first, then the rest of `/etc/shells` that exists, one entry per program name.
#[cfg(any(not(windows), test))]
fn unix_shells(
    login: Option<&Path>,
    etc_shells: &str,
    exists: impl Fn(&Path) -> bool,
) -> Vec<Shell> {
    let listed = etc_shells
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('/'))
        .map(PathBuf::from);
    let mut found = Vec::new();
    for program in login.map(Path::to_path_buf).into_iter().chain(listed) {
        let Some(name) = program.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if matches!(name, "nologin" | "false" | "true") || !exists(&program) {
            continue;
        }
        let name = name.to_owned();
        found.push(Shell {
            id: name.clone(),
            name,
            program,
            args: Vec::new(),
        });
    }
    dedupe(found)
}

fn dedupe(shells: Vec<Shell>) -> Vec<Shell> {
    let mut seen = HashSet::new();
    shells
        .into_iter()
        .filter(|shell| seen.insert(shell.id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        bytes
    }

    #[test]
    fn wsl_distributions_are_read_from_utf16_output() {
        let output = utf16("Ubuntu-22.04\r\ndocker-desktop\r\nDebian\r\n\r\n");
        assert_eq!(decode_wsl_list(&output), ["Ubuntu-22.04", "Debian"]);
        assert_eq!(decode_wsl_list(b"Alpine\n"), ["Alpine"]);
        assert!(decode_wsl_list(&[]).is_empty());
    }

    #[test]
    fn unix_shells_start_with_the_login_shell_and_skip_duplicates_and_nologin() {
        let etc = "# shells\n/bin/sh\n/bin/bash\n/usr/bin/bash\n/usr/bin/zsh\n/usr/sbin/nologin\n/bin/missing\n";
        let found = unix_shells(Some(Path::new("/usr/bin/zsh")), etc, |path| {
            !path.ends_with("missing")
        });
        let names: Vec<_> = found.iter().map(|shell| shell.name.as_str()).collect();
        assert_eq!(names, ["zsh", "sh", "bash"]);
        assert_eq!(found[0].program, Path::new("/usr/bin/zsh"));
    }
}
