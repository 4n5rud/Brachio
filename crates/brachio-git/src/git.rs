//! A thin wrapper that runs the git CLI in one repository.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

/// Long enough for a push or a commit hook, short enough that a stuck git is noticed.
const GIT_TIME_LIMIT: Duration = Duration::from_secs(300);

/// Why a git command failed.
#[derive(Debug, thiserror::Error)]
#[must_use]
pub enum GitError {
    /// git could not be started, usually because it is not installed.
    #[error("could not run git: {0}")]
    Spawn(#[from] io::Error),
    /// git ran and exited with a failure status.
    #[error("git {args} failed with status {status}: {stderr}")]
    Failed {
        /// The arguments, joined for display only.
        args: String,
        /// Exit status, or -1 when the process was killed by a signal.
        status: i32,
        /// Trimmed standard error.
        stderr: String,
    },
    /// The folder is not inside a git repository.
    #[error("{} is not a git repository.{}", .path.display(), did_you_mean(.suggestions))]
    NotARepository {
        /// The folder that was asked about.
        path: PathBuf,
        /// Repositories directly inside that folder, which the user may have meant.
        suggestions: Vec<PathBuf>,
    },
    /// The repository has a remote but names no default branch, and has no `main` or `master`.
    #[error("Could not tell which branch new tasks should start from in {}. Run `git remote set-head origin --auto` in it, then add it again.", .path.display())]
    NoDefaultBranch {
        /// The repository folder.
        path: PathBuf,
    },
}

fn did_you_mean(suggestions: &[PathBuf]) -> String {
    if suggestions.is_empty() {
        return " Choose the folder that contains .git.".to_owned();
    }
    let names: Vec<String> = suggestions
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    format!(" Did you mean: {}?", names.join(", "))
}

/// A git CLI bound to one repository directory.
#[derive(Debug, Clone)]
pub struct Git {
    repo: PathBuf,
}

impl Git {
    /// Binds a git CLI to `repo`.
    #[must_use]
    pub fn new(repo: &Path) -> Self {
        Self {
            repo: repo.to_path_buf(),
        }
    }

    /// The repository directory.
    #[must_use]
    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// Runs git and returns its raw output, whatever the exit status.
    ///
    /// # Errors
    /// Fails only when git cannot be started.
    pub fn output(&self, args: &[&str]) -> Result<Output, GitError> {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.repo);
        if cfg!(windows) {
            command.args(["-c", "core.longpaths=true"]);
        }
        // A credential prompt nobody can see would otherwise hold this thread forever.
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .args(args);
        Ok(brachio_process::output_within(
            &mut command,
            GIT_TIME_LIMIT,
        )?)
    }

    /// Runs git and fails on a non-zero exit. Returns trimmed stdout.
    ///
    /// # Errors
    /// Fails when git cannot be started or exits with a failure status.
    pub fn run(&self, args: &[&str]) -> Result<String, GitError> {
        Ok(self.run_text(args)?.trim().to_owned())
    }

    /// Runs git and preserves whitespace in successful stdout.
    ///
    /// # Errors
    /// Fails when git cannot be started or exits with a failure status.
    pub fn run_text(&self, args: &[&str]) -> Result<String, GitError> {
        let output = self.output(args)?;
        if !output.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}
