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
    /// git succeeded but printed something that cannot be trusted.
    #[error("{0}")]
    UnexpectedOutput(String),
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
    envs: Vec<(String, String)>,
}

impl Git {
    /// Binds a git CLI to `repo`.
    #[must_use]
    pub fn new(repo: &Path) -> Self {
        Self {
            repo: repo.to_path_buf(),
            envs: Vec::new(),
        }
    }

    /// The same CLI with one more environment variable set on every command it runs.
    #[must_use]
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.envs.push((key.to_owned(), value.to_owned()));
        self
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
        let mut command = self.command(args);
        Ok(brachio_process::output_within(
            &mut command,
            GIT_TIME_LIMIT,
        )?)
    }

    /// Runs git with `input` on its standard input and returns its trimmed standard output.
    ///
    /// # Errors
    /// Fails when git cannot be started or exits with a failure status.
    pub fn run_with_input(&self, args: &[&str], input: &[u8]) -> Result<String, GitError> {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            // A git that exits early closes the pipe; its own exit status says why.
            let _ = stdin.write_all(input);
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.repo);
        if cfg!(windows) {
            command.args(["-c", "core.longpaths=true"]);
        }
        // A credential prompt nobody can see would otherwise hold this thread forever.
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .envs(self.envs.iter().map(|(key, value)| (key, value)))
            .args(args);
        command
    }

    /// Runs git and keeps at most `max_bytes` of its output, stopping git once that is exceeded.
    /// Returns the text and whether it was cut. A cut can land inside a line.
    ///
    /// # Errors
    /// Fails when git cannot be started, or exits with a failure before the cap is reached.
    pub fn run_text_capped(
        &self,
        args: &[&str],
        max_bytes: usize,
    ) -> Result<(String, bool), GitError> {
        let (bytes, truncated) = self.run_bytes_capped(args, max_bytes)?;
        // A cut can split a multi-byte character, which becomes one replacement mark.
        Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
    }

    pub(crate) fn run_bytes_capped(
        &self,
        args: &[&str],
        max_bytes: usize,
    ) -> Result<(Vec<u8>, bool), GitError> {
        let mut command = self.command(args);
        let capped = brachio_process::output_capped(&mut command, GIT_TIME_LIMIT, max_bytes)?;
        let output = capped.output;
        if !capped.truncated && !output.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        let mut bytes = output.stdout;
        bytes.truncate(max_bytes);
        Ok((bytes, capped.truncated))
    }

    /// Runs git and passes its stdout to `on_chunk` as it arrives, so a huge answer is never
    /// held in memory.
    ///
    /// # Errors
    /// Fails when git cannot be started, runs too long, or exits with a failure status.
    pub fn stream_stdout(
        &self,
        args: &[&str],
        on_chunk: impl FnMut(&[u8]),
    ) -> Result<(), GitError> {
        let mut command = self.command(args);
        let streamed = brachio_process::stream_stdout(&mut command, GIT_TIME_LIMIT, on_chunk)?;
        if !streamed.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                status: streamed.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&streamed.stderr).trim().to_owned(),
            });
        }
        Ok(())
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
