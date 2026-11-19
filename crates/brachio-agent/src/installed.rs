//! Finding agent CLIs on this machine and building the command that starts
//! them.

use std::env;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// An agent CLI that cannot print its version in this time is not worth waiting for.
const VERSION_TIME_LIMIT: Duration = Duration::from_secs(5);
const BATCH_COMMAND: &str = "BRACHIO_AGENT_BATCH_COMMAND";

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
    /// Child-only environment overrides.
    pub env: Vec<(String, String)>,
}

impl Installed {
    /// `launch` resolved against this install, ready for a PTY.
    ///
    /// # Errors
    /// Fails when an argument cannot be passed safely through a batch shim.
    pub fn resolve(&self, launch: &Launch) -> io::Result<ResolvedCommand> {
        ResolvedCommand::new(&self.path, &launch.args)
    }

    /// The interactive agent, with a first prompt when it can be passed
    /// safely. Through `cmd.exe`, text with shell characters cannot, so the
    /// agent then starts without it and the caller should type it in.
    ///
    /// `session_id` names the new conversation, for agents that accept one.
    ///
    /// # Errors
    /// Fails when a batch shim cannot safely receive the options.
    pub fn resolve_interactive(
        &self,
        prompt: Option<&str>,
        session_id: Option<&str>,
        mut options: Vec<String>,
    ) -> io::Result<(ResolvedCommand, PromptDelivery)> {
        let prompt = prompt.filter(|text| !text.trim().is_empty());
        let (mut launch, delivery) = match prompt {
            None => (self.kind.interactive(), PromptDelivery::None),
            Some(text) if is_batch_file(&self.path) && !is_safe_for_cmd(text) => {
                (self.kind.interactive(), PromptDelivery::TypeIn)
            }
            Some(text) => (
                self.kind.interactive_with_prompt(text),
                PromptDelivery::Argument,
            ),
        };
        if let Some(id) = session_id {
            options.extend(self.kind.session_id_args(id));
        }
        options.append(&mut launch.args);
        launch.args = options;
        Ok((self.resolve(&launch)?, delivery))
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
    const SPECIAL: &[char] = &['"', '%', '^', '&', '|', '<', '>', '!'];
    !text.contains(SPECIAL) && !text.chars().any(char::is_control)
}

impl ResolvedCommand {
    /// Builds the command that starts `path` with `args`.
    ///
    /// Windows batch files go through `cmd.exe`.
    ///
    /// # Errors
    /// Fails if the path or arguments cannot be represented safely for CMD.
    pub fn new(path: &Path, args: &[String]) -> io::Result<Self> {
        if !is_batch_file(path) {
            return Ok(Self {
                program: path.to_path_buf(),
                args: args.to_vec(),
                env: Vec::new(),
            });
        }
        let path = path
            .to_str()
            .filter(|path| !path.contains('"') && !path.chars().any(char::is_control))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Unsupported agent batch path")
            })?;
        if args.iter().any(|arg| !is_safe_for_cmd(arg)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unsupported agent batch argument",
            ));
        }
        // CMD expands this token after processing /c; both process launchers can
        // pass the token verbatim without CRT-style escaping of the inner quotes.
        let mut line = format!("\"{path}\"");
        for arg in args {
            line.push_str(" \"");
            line.push_str(arg);
            // Batch shims generally forward %* to a native program. Preserve
            // trailing backslashes when that program parses its quoted argv.
            for _ in arg.bytes().rev().take_while(|byte| *byte == b'\\') {
                line.push('\\');
            }
            line.push('"');
        }
        Ok(Self {
            program: PathBuf::from("cmd.exe"),
            args: vec![
                "/d".into(),
                "/v:off".into(),
                "/c".into(),
                format!("%{BATCH_COMMAND}%"),
            ],
            env: vec![(BATCH_COMMAND.to_owned(), line)],
        })
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

/// Suffixes to try after a program name. On Windows, use executable extensions
/// so an extensionless npm shell shim cannot shadow its Windows counterpart.
fn executable_extensions(pathext_var: Option<&OsStr>) -> Vec<String> {
    if cfg!(windows) {
        let pathext = pathext_var
            .filter(|value| !value.is_empty())
            .map_or_else(|| ".COM;.EXE;.BAT;.CMD".into(), OsStr::to_string_lossy);
        return pathext
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
    }
    vec![String::new()]
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
    let command = ResolvedCommand::new(path, &["--version".to_owned()]).ok()?;
    let output = brachio_process::output_within(
        Command::new(&command.program)
            .args(&command.args)
            .envs(command.env),
        VERSION_TIME_LIMIT,
    )
    .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?.trim();
    (!first_line.is_empty()).then(|| first_line.to_owned())
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::ffi::OsStr;
    use std::fs;

    use super::{find_on_path, read_version, Installed, PromptDelivery};
    use crate::AgentKind;

    #[test]
    fn npm_shell_shim_does_not_shadow_the_windows_launcher() {
        let bin = tempfile::tempdir().expect("create PATH directory");
        fs::write(bin.path().join("codex"), "#!/bin/sh\n").expect("write Unix shim");
        let launcher = bin.path().join("codex.cmd");
        fs::write(&launcher, "@echo off\r\n").expect("write Windows shim");

        let found = find_on_path(
            "codex",
            Some(bin.path().as_os_str()),
            Some(OsStr::new(".EXE;.CMD")),
        );

        assert_eq!(found, Some(launcher));
    }

    #[test]
    fn runs_a_spaced_batch_shim_and_preserves_its_arguments() {
        let root = tempfile::tempdir().expect("create temporary directory");
        let bin = root.path().join("npm bin (test)");
        fs::create_dir(&bin).expect("create shim directory");
        let path = bin.join("claude.cmd");
        fs::write(&path, "@echo off\r\necho %~1\r\n").expect("write version fixture");
        assert_eq!(read_version(&path).as_deref(), Some("--version"));
        fs::write(
            &path,
            "@echo off\r\necho [%~1][%~2][%~3][%~4][%~5][%~6]\r\n",
        )
        .expect("write argument fixture");
        let installed = Installed {
            kind: AgentKind::ClaudeCode,
            path,
            version: None,
        };
        let (command, delivery) = installed
            .resolve_interactive(
                Some("- summarize this"),
                Some("session"),
                vec![
                    "--settings".to_owned(),
                    "C:\\test data\\settings.json".to_owned(),
                ],
            )
            .expect("resolve batch command");
        let output = std::process::Command::new(command.program)
            .args(command.args)
            .envs(command.env)
            .output()
            .expect("run batch command");
        assert!(output.status.success());
        assert_eq!(delivery, PromptDelivery::Argument);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(),
            "[--settings][C:\\test data\\settings.json][--session-id][session][--][- summarize this]");
    }
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

        let (_, plain) = installed
            .resolve_interactive(Some("add a test"), None, vec![])
            .expect("resolve");
        let (command, risky) = installed
            .resolve_interactive(Some("fix it & push"), None, vec![])
            .expect("resolve");

        assert_eq!(plain, PromptDelivery::Argument);
        assert_eq!(risky, PromptDelivery::TypeIn);
        assert_eq!(command.env[0].1, "\"C:/npm/claude.cmd\"");
    }

    #[test]
    fn session_id_comes_before_the_prompt() {
        let installed = Installed {
            kind: AgentKind::ClaudeCode,
            path: PathBuf::from("/usr/bin/claude"),
            version: None,
        };

        let (command, _) = installed
            .resolve_interactive(Some("add a test"), Some("abc"), vec![])
            .expect("resolve");

        assert_eq!(command.args, vec!["--session-id", "abc", "add a test"]);
    }

    #[test]
    fn batch_files_run_through_cmd() {
        let batch = ResolvedCommand::new(Path::new("C:/npm/claude.cmd"), &["--version".to_owned()])
            .expect("resolve");
        assert_eq!(batch.program, PathBuf::from("cmd.exe"));
        assert_eq!(
            batch.args,
            vec!["/d", "/v:off", "/c", "%BRACHIO_AGENT_BATCH_COMMAND%"]
        );

        let direct = ResolvedCommand::new(Path::new("/usr/bin/codex"), &[]).expect("resolve");
        assert_eq!(direct.program, PathBuf::from("/usr/bin/codex"));
        assert!(direct.args.is_empty(), "no arguments were given");
    }
}
