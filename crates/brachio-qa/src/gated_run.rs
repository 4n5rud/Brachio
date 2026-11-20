//! The project's `[qa]` command, shown to the user before it runs, and the result it leaves.
//!
//! ```toml
//! [qa]
//! program = "C:/Program Files/nodejs/npm.cmd"   # absolute path to an executable
//! args = ["test"]
//! timeout_secs = 600
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use brachio_git::local_head;
use serde::{Deserialize, Serialize};

use crate::{run_approved, QaCommand, QaExit, QaNotRun, VerifiedQaRun};

const CONFIG_FILE: &str = "brachio.toml";
const DEFAULT_TIMEOUT_SECS: u64 = 600;

#[derive(Default, Deserialize)]
struct RepoConfig {
    qa: Option<QaSection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QaSection {
    program: PathBuf,
    #[serde(default)]
    args: Vec<String>,
    timeout_secs: Option<u64>,
}

/// Reads `[qa]` of `<repo_root>/brachio.toml`. `None` when the project has no QA command.
///
/// # Errors
/// Fails when the file cannot be read, is not valid, or the command is not allowed.
pub fn read_command(repo_root: &Path, worktree: &Path) -> Result<Option<QaCommand>, String> {
    let path = repo_root.join(CONFIG_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let config: RepoConfig = toml::from_str(&text)
        .map_err(|error| format!("{}: {}", path.display(), error.message()))?;
    let Some(section) = config.qa else {
        return Ok(None);
    };
    if section
        .args
        .iter()
        .any(|arg| arg.chars().any(char::is_control))
    {
        return Err("QA arguments must be printable text.".to_owned());
    }
    let timeout = Duration::from_secs(section.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));
    QaCommand::new(
        section.program,
        section.args.into_iter().map(Into::into).collect(),
        worktree,
        timeout,
    )
    .map(Some)
    .map_err(|error| format!("The QA command in brachio.toml is not usable: {error}"))
}

/// The exact text the user approves: the command line, where it runs, and the limit.
#[must_use]
pub fn review_text(command: &QaCommand, sha: &str) -> String {
    format!(
        "Run QA\ncommand: {}\ndirectory: {}\ncommit: {sha}\ntime limit: {} s",
        command_line(command),
        command.cwd().display(),
        command.timeout().as_secs()
    )
}

fn command_line(command: &QaCommand) -> String {
    command
        .argv()
        .iter()
        .map(|part| part.to_string_lossy())
        .map(|part| {
            if part.is_empty() || part.contains(char::is_whitespace) {
                format!("\"{part}\"")
            } else {
                part.into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// How the run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QaOutcome {
    /// Exit status 0.
    Passed,
    /// Non-zero exit status.
    Failed,
    /// The time limit ended the run.
    TimedOut,
    /// Output exceeded the cap.
    OutputLimit,
    /// The command could not be started.
    ProcessError,
}

/// What the UI shows for the last run of a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QaReport {
    /// The command line that ran.
    pub command: String,
    /// The directory it ran in.
    pub directory: String,
    /// How it ended.
    pub outcome: QaOutcome,
    /// Exit code, when the process exited with one.
    pub exit_code: Option<i32>,
    /// Run time in milliseconds.
    pub duration_ms: u64,
    /// The commit that was checked out when the run started.
    pub sha: String,
    /// Brachio ran the command itself and the same commit was still checked out afterwards.
    /// An agent's own claim that tests passed is never this.
    pub verified_by_brachio: bool,
    /// Captured standard output, capped.
    pub stdout: String,
    /// Captured standard error, capped.
    pub stderr: String,
    /// Some output was dropped.
    pub truncated: bool,
}

fn report(command: &QaCommand, run: &VerifiedQaRun) -> QaReport {
    let (outcome, exit_code) = match run.exit {
        QaExit::Passed => (QaOutcome::Passed, Some(0)),
        QaExit::Failed(status) => (QaOutcome::Failed, status.code()),
        QaExit::TimedOut => (QaOutcome::TimedOut, None),
        QaExit::OutputLimit => (QaOutcome::OutputLimit, None),
        QaExit::ProcessError(_) => (QaOutcome::ProcessError, None),
    };
    QaReport {
        command: command_line(command),
        directory: command.cwd().display().to_string(),
        outcome,
        exit_code,
        duration_ms: u64::try_from(run.elapsed.as_millis()).unwrap_or(u64::MAX),
        sha: run.sha.clone(),
        verified_by_brachio: run.head_unchanged && outcome != QaOutcome::ProcessError,
        stdout: run.stdout.text.clone(),
        stderr: run.stderr.text.clone(),
        truncated: run.stdout.truncated || run.stderr.truncated,
    }
}

/// Why nothing ran, or the result of the one run.
#[derive(Debug, PartialEq, Eq)]
pub enum GatedOutcome {
    /// The user said no, or could not be asked.
    Declined,
    /// The run happened.
    Ran(Box<QaReport>),
}

/// Runs `command` only if `approve` returns true for the exact text shown. `approve` blocks until
/// the user answers. There is no retry and nothing runs afterwards.
///
/// # Errors
/// When HEAD cannot be read or changed while the user was deciding.
pub fn run_after_approval(
    command: &QaCommand,
    redact: &[&str],
    approve: impl FnOnce(&str) -> bool,
) -> Result<GatedOutcome, String> {
    let sha = local_head(command.cwd()).map_err(|error| error.to_string())?;
    if !approve(&review_text(command, &sha)) {
        return Ok(GatedOutcome::Declined);
    }
    match run_approved(Some(command), Some(command.approval(sha)), redact) {
        Ok(run) => Ok(GatedOutcome::Ran(Box::new(report(command, &run)))),
        Err(QaNotRun::StaleHead) => {
            Err("The checked-out commit changed while waiting. Run QA again.".to_owned())
        }
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::process::Command;

    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("dir");
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "x",
            ],
        ] {
            assert!(Command::new("git")
                .args(&args)
                .current_dir(dir.path())
                .status()
                .expect("git")
                .success());
        }
        dir
    }

    fn command(dir: &Path) -> QaCommand {
        let program = std::env::current_exe().expect("exe");
        QaCommand::new(program, vec!["--list".into()], dir, Duration::from_secs(60))
            .expect("command")
    }

    #[test]
    fn a_declined_approval_never_runs_the_command() {
        let dir = repo();
        let command = command(dir.path());
        let shown = Cell::new(String::new());
        let outcome = run_after_approval(&command, &[], |text| {
            shown.set(text.to_owned());
            false
        })
        .expect("no error");
        assert_eq!(outcome, GatedOutcome::Declined);
        let shown = shown.take();
        assert!(shown.contains("--list"), "{shown}");
        assert!(shown.contains(
            &dir.path()
                .canonicalize()
                .expect("path")
                .display()
                .to_string()
        ));
    }

    #[test]
    fn an_approved_run_reports_status_commit_and_verification() {
        let dir = repo();
        let command = command(dir.path());
        let GatedOutcome::Ran(report) = run_after_approval(&command, &[], |_| true).expect("ran")
        else {
            panic!("expected a run");
        };
        assert_eq!(report.outcome, QaOutcome::Passed);
        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.sha, local_head(dir.path()).expect("head"));
        assert!(report.verified_by_brachio);
    }

    #[test]
    fn the_project_command_comes_from_brachio_toml_or_is_absent() {
        let dir = repo();
        assert_eq!(read_command(dir.path(), dir.path()), Ok(None));
        let exe = std::env::current_exe().expect("exe");
        let text = format!(
            "[qa]\nprogram = {:?}\nargs = [\"--list\"]\n",
            exe.display().to_string()
        );
        fs::write(dir.path().join(CONFIG_FILE), text).expect("write");
        let found = read_command(dir.path(), dir.path()).expect("read");
        assert_eq!(found.expect("a command").timeout().as_secs(), 600);
        fs::write(dir.path().join(CONFIG_FILE), "[qa]\nprogram = \"npm\"\n").expect("write");
        assert!(read_command(dir.path(), dir.path()).is_err());
    }
}
