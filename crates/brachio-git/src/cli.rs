use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("could not run git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git {args} failed with status {status}: {stderr}")]
    Failed {
        args: String,
        status: i32,
        stderr: String,
    },
}

/// A git CLI bound to one repository directory.
#[derive(Debug, Clone)]
pub struct Git {
    repo: PathBuf,
}

impl Git {
    #[must_use]
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self { repo: repo.into() }
    }

    #[must_use]
    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// Runs git and returns its raw output, whatever the exit status.
    pub fn output(&self, args: &[&str]) -> Result<Output, GitError> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.repo);
        if cfg!(windows) {
            cmd.args(["-c", "core.longpaths=true"]);
        }
        Ok(cmd.args(args).output()?)
    }

    /// Runs git and fails on a non-zero exit. Returns trimmed stdout.
    pub fn run(&self, args: &[&str]) -> Result<String, GitError> {
        let out = self.output(args)?;
        if !out.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                status: out.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }
}
