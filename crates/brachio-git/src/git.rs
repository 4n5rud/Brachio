//! A thin wrapper that runs the git CLI in one repository.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
        Ok(command.args(args).output()?)
    }

    /// Runs git and fails on a non-zero exit. Returns trimmed stdout.
    ///
    /// # Errors
    /// Fails when git cannot be started or exits with a failure status.
    pub fn run(&self, args: &[&str]) -> Result<String, GitError> {
        let output = self.output(args)?;
        if !output.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}
