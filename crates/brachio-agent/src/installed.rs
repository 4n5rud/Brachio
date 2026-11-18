//! Finding agent CLIs on this machine and building the command that starts
//! them.

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::agent_kind::{AgentKind, Launch};

/// An agent CLI found on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Installed {
    /// Which agent this is.
    pub kind: AgentKind,
    /// Full path of the executable.
    pub path: PathBuf,
    /// First line of `--version`, when the CLI answered.
    pub version: Option<String>,
}

/// The program and arguments that actually start an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommand {
    /// Program to execute.
    pub program: PathBuf,
    /// Arguments, one element per argument.
    pub args: Vec<String>,
}

impl Installed {
    /// `launch` resolved against this install, ready for a PTY.
    #[must_use]
    pub fn resolve(&self, launch: &Launch) -> ResolvedCommand {
        ResolvedCommand::new(&self.path, &launch.args)
    }

    /// The interactive agent, with a first prompt when it can be passed
    /// safely. Through `cmd.exe`, text with shell characters cannot, so the
    /// agent then starts without it and the caller should type it in.
    #[must_use]
    pub fn resolve_interactive(&self, prompt: Option<&str>) -> (ResolvedCommand, PromptDelivery) {
        let Some(prompt) = prompt.filter(|text| !text.trim().is_empty()) else {
            return (self.resolve(&self.kind.interactive()), PromptDelivery::None);
        };
        if is_batch_file(&self.path) && !is_safe_for_cmd(prompt) {
            return (
                self.resolve(&self.kind.interactive()),
                PromptDelivery::TypeIn,
            );
        }
        let launch = self.kind.interactive_with_prompt(prompt);
        (self.resolve(&launch), PromptDelivery::Argument)
    }
}

/// How the first prompt reaches the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptDelivery {
    /// There was no prompt.
    None,
    /// The prompt is a command-line argument.
    Argument,
    /// The caller must type the prompt into the terminal.
    TypeIn,
}

/// True when `cmd.exe` passes `text` through unchanged inside double quotes.
fn is_safe_for_cmd(text: &str) -> bool {
    const SPECIAL: &[char] = &['"', '%', '^', '&', '|', '<', '>', '!', '\n', '\r'];
    !text.contains(SPECIAL)
}

impl ResolvedCommand {
    /// Builds the command that starts `path` with `args`.
    ///
    /// Windows batch files (`.cmd`, `.bat`) cannot be started directly, so
    /// they go through `cmd.exe`. `cmd.exe` re-parses the arguments, so only
    /// fixed flags may go this way. Prompts and other free text need the
    /// agent's real entry point instead.
    #[must_use]
    pub fn new(path: &Path, args: &[String]) -> Self {
        if !is_batch_file(path) {
            return Self {
                program: path.to_path_buf(),
                args: args.to_vec(),
            };
        }
        let mut cmd_args = vec![
            "/d".to_owned(),
            "/c".to_owned(),
            path.to_string_lossy().into_owned(),
        ];
        cmd_args.extend_from_slice(args);
        Self {
            program: PathBuf::from("cmd.exe"),
            args: cmd_args,
        }
    }
}

/// Looks up every supported agent on `PATH`.
#[must_use]
pub fn detect_all() -> Vec<Installed> {
    AgentKind::ALL.into_iter().filter_map(detect).collect()
}

/// Finds `kind` on `PATH` and asks it for its version.
#[must_use]
pub fn detect(kind: AgentKind) -> Option<Installed> {
    let path_var = env::var_os("PATH");
    let pathext_var = env::var_os("PATHEXT");
    let path = find_on_path(kind.program(), path_var.as_deref(), pathext_var.as_deref())?;
    let version = read_version(&path);
    Some(Installed {
        kind,
        path,
        version,
    })
}

/// Resolves `program` against `path_var` the way a shell would. On Windows,
/// each `PATHEXT` extension is tried too, since npm installs `claude.cmd`.
#[must_use]
pub fn find_on_path(
    program: &str,
    path_var: Option<&OsStr>,
    pathext_var: Option<&OsStr>,
) -> Option<PathBuf> {
    let extensions = executable_extensions(pathext_var);
    for dir in env::split_paths(path_var?) {
        for extension in &extensions {
            let candidate = dir.join(format!("{program}{extension}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Suffixes to try after a program name. The bare name always comes first.
fn executable_extensions(pathext_var: Option<&OsStr>) -> Vec<String> {
    let mut extensions = vec![String::new()];
    if cfg!(windows) {
        let pathext = pathext_var.map_or_else(
            || ".COM;.EXE;.BAT;.CMD".to_owned(),
            |value| value.to_string_lossy().into_owned(),
        );
        let listed = pathext.split(';').filter(|extension| !extension.is_empty());
        extensions.extend(listed.map(str::to_ascii_lowercase));
    }
    extensions
}

fn is_batch_file(path: &Path) -> bool {
    let extension = path.extension().and_then(OsStr::to_str).unwrap_or("");
    extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = path.metadata() else {
        return false;
    };
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn read_version(path: &Path) -> Option<String> {
    let command = ResolvedCommand::new(path, &["--version".to_owned()]);
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?.trim();
    (!first_line.is_empty()).then(|| first_line.to_owned())
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("create temp dir")
    }

    #[test]
    fn finds_an_executable_and_skips_plain_files() {
        let plain = temp_dir();
        let bin = temp_dir();
        fs::write(plain.path().join("claude"), "").expect("write plain file");
        let executable = bin.path().join("claude");
        fs::write(&executable, "#!/bin/sh\necho 1.2.3\n").expect("write script");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).expect("chmod");
        let path_var = env::join_paths([plain.path(), bin.path()]).expect("join PATH");

        let found = find_on_path("claude", Some(&path_var), None);

        assert_eq!(found, Some(executable.clone()));
        assert_eq!(read_version(&executable).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn missing_program_is_none() {
        let empty = temp_dir();
        let path_var = env::join_paths([empty.path()]).expect("join PATH");
        assert_eq!(find_on_path("codex", Some(&path_var), None), None);
        assert_eq!(find_on_path("codex", None, None), None);
    }

    #[test]
    fn prompts_with_shell_characters_are_typed_in_through_cmd() {
        let installed = Installed {
            kind: AgentKind::ClaudeCode,
            path: PathBuf::from("C:/npm/claude.cmd"),
            version: None,
        };

        let (_, plain) = installed.resolve_interactive(Some("add a test"));
        let (command, risky) = installed.resolve_interactive(Some("fix it & push"));

        assert_eq!(plain, PromptDelivery::Argument);
        assert_eq!(risky, PromptDelivery::TypeIn);
        assert_eq!(command.args, vec!["/d", "/c", "C:/npm/claude.cmd"]);
    }

    #[test]
    fn batch_files_run_through_cmd() {
        let batch = ResolvedCommand::new(Path::new("C:/npm/claude.cmd"), &["--version".to_owned()]);
        assert_eq!(batch.program, PathBuf::from("cmd.exe"));
        assert_eq!(
            batch.args,
            vec!["/d", "/c", "C:/npm/claude.cmd", "--version"]
        );

        let direct = ResolvedCommand::new(Path::new("/usr/bin/codex"), &[]);
        assert_eq!(direct.program, PathBuf::from("/usr/bin/codex"));
        assert!(direct.args.is_empty(), "no arguments were given");
    }
}
