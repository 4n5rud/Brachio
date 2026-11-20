//! Run only the exact project command approved for the current worktree HEAD.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};

use brachio_git::local_head;
use brachio_process::output_capped;
use thiserror::Error;

use crate::{failure_signature, FeedbackDecision, FeedbackGate};

const OUTPUT_BYTES: usize = 32 * 1024;
const MAX_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// One opt-in command loaded by the app from trusted project configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QaCommand {
    argv: Vec<OsString>,
    cwd: PathBuf,
    timeout: Duration,
}

impl QaCommand {
    /// Resolve the working directory before displaying the approval. The program
    /// must already have an absolute path so launch does not repeat a PATH lookup.
    ///
    /// # Errors
    /// Returns an error if the command path, directory, or timeout is invalid.
    pub fn new(
        program: PathBuf,
        args: Vec<OsString>,
        cwd: &Path,
        timeout: Duration,
    ) -> io::Result<Self> {
        if !program.is_absolute()
            || (cfg!(windows)
                && !program
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe")))
            || timeout.is_zero()
            || timeout > MAX_TIMEOUT
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid QA program or timeout",
            ));
        }
        let cwd = fs::canonicalize(cwd)?;
        if !cwd.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "QA working directory is not a directory",
            ));
        }
        let mut argv = vec![program.into_os_string()];
        argv.extend(args);
        Ok(Self { argv, cwd, timeout })
    }

    /// Exact executable and arguments to show before approval.
    #[must_use]
    pub fn argv(&self) -> &[OsString] {
        &self.argv
    }

    /// Canonical working directory to show before approval.
    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Runtime limit to show before approval.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Create the approval only after a person accepts this exact argv, cwd and
    /// HEAD. The caller owns the confirmation UI and must keep approvals local.
    #[must_use]
    pub fn approval(&self, sha: String) -> QaApproval {
        QaApproval {
            command: self.clone(),
            sha,
        }
    }
}

/// A local, single-use approval bound to a command and Git HEAD.
#[derive(Debug)]
pub struct QaApproval {
    command: QaCommand,
    sha: String,
}

/// Why the project command was never started.
#[derive(Debug, Error)]
pub enum QaNotRun {
    #[error("project QA is disabled")]
    /// Project configuration has no enabled QA command.
    Disabled,
    #[error("the current QA command has no matching user approval")]
    /// No approval was provided for this version of the command.
    NotApproved,
    #[error("cannot read QA worktree HEAD: {0}")]
    /// Git could not read the current HEAD.
    Head(#[from] brachio_git::StatusError),
    #[error("the approved QA commit is no longer checked out")]
    /// HEAD changed after approval and before execution.
    StaleHead,
}

/// Outcome of running the approved command.
#[derive(Debug, PartialEq, Eq)]
pub enum QaExit {
    /// The program exited successfully.
    Passed,
    /// The program exited unsuccessfully or was terminated by a signal.
    Failed(ExitStatus),
    /// Execution or collection of its output exceeded the time limit.
    TimedOut,
    /// Standard output exceeded the cap and ended the run.
    OutputLimit,
    /// The process could not be started or observed.
    ProcessError(io::ErrorKind),
}

/// One bounded stream of QA output.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct QaOutput {
    /// The bounded text after known secrets have been replaced.
    pub text: String,
    /// Some captured bytes could not be retained.
    pub truncated: bool,
}

/// Process observation made by Brachio. The SHA is checked at launch and exit;
/// it identifies HEAD and does not assert that the worktree is clean or immutable.
#[derive(Debug)]
pub struct VerifiedQaRun {
    /// The Git HEAD read immediately before starting the command.
    pub sha: String,
    /// Whether Git reported the same HEAD after execution.
    pub head_unchanged: bool,
    /// How the approved execution ended.
    pub exit: QaExit,
    /// Elapsed time spent executing and collecting process output.
    pub elapsed: Duration,
    /// Captured standard output.
    pub stdout: QaOutput,
    /// Captured standard error.
    pub stderr: QaOutput,
    failure_signature: Option<u64>,
}

impl VerifiedQaRun {
    /// Optional caller-driven decision. This never runs another command or sends a message.
    pub fn feedback(&mut self, gate: &mut FeedbackGate) -> Option<FeedbackDecision> {
        self.failure_signature
            .take()
            .map(|signature| gate.decide(signature))
    }
}

/// Runs at most one approval-bound command. Explicit redactions are literal
/// secret values supplied by the caller and are removed before returning logs.
///
/// # Errors
/// Rejects disabled, unapproved, and stale requests before starting the command.
pub fn run_approved(
    configured: Option<&QaCommand>,
    approval: Option<QaApproval>,
    redact: &[&str],
) -> Result<VerifiedQaRun, QaNotRun> {
    let configured = configured.ok_or(QaNotRun::Disabled)?;
    let approval = approval
        .filter(|approval| &approval.command == configured)
        .ok_or(QaNotRun::NotApproved)?;
    let sha = local_head(&configured.cwd)?;
    if sha != approval.sha {
        return Err(QaNotRun::StaleHead);
    }

    let [program, args @ ..] = configured.argv.as_slice() else {
        unreachable!("QaCommand construction always includes the executable")
    };
    let started = Instant::now();
    let result = output_capped(
        Command::new(program)
            .args(args)
            .current_dir(&configured.cwd),
        configured.timeout,
        OUTPUT_BYTES,
    );
    let elapsed = started.elapsed();
    let head_unchanged = local_head(&configured.cwd).is_ok_and(|head| head == sha);
    let (exit, stdout, stderr) = match result {
        Ok(capped) => {
            let status = capped.output.status;
            let exit = if capped.truncated {
                QaExit::OutputLimit
            } else if status.success() {
                QaExit::Passed
            } else {
                QaExit::Failed(status)
            };
            let stdout = redact_output(&capped.output.stdout, redact);
            let stderr = redact_output(&capped.output.stderr, redact);
            (exit, stdout, stderr)
        }
        Err(error) => {
            let exit = if error.kind() == io::ErrorKind::TimedOut {
                QaExit::TimedOut
            } else {
                QaExit::ProcessError(error.kind())
            };
            (exit, QaOutput::default(), QaOutput::default())
        }
    };

    let failure_signature = match exit {
        QaExit::Failed(status) if head_unchanged => {
            let first_error = stderr
                .text
                .lines()
                .chain(stdout.text.lines())
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            Some(failure_signature(
                &format!("{:?} {status}", configured.argv),
                first_error,
            ))
        }
        _ => None,
    };
    Ok(VerifiedQaRun {
        sha,
        head_unchanged,
        exit,
        elapsed,
        stdout,
        stderr,
        failure_signature,
    })
}

fn redact_output(bytes: &[u8], redact: &[&str]) -> QaOutput {
    let mut truncated = bytes.len() > OUTPUT_BYTES;
    let bytes = &bytes[..bytes.len().min(OUTPUT_BYTES)];
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    let mut secrets: Vec<&str> = redact
        .iter()
        .copied()
        .filter(|secret| !secret.is_empty())
        .collect();
    secrets.sort_unstable_by_key(|secret| std::cmp::Reverse(secret.len()));
    for secret in secrets {
        if truncated {
            // A cap can split a secret before its final byte. Remove a matching
            // prefix at the end before the normal whole-value redaction.
            let limit = secret.len().min(text.len());
            if let Some(size) = (1..limit).rev().find(|size| {
                secret
                    .get(..*size)
                    .is_some_and(|prefix| text.ends_with(prefix))
            }) {
                text.truncate(text.len() - size);
                text.push_str("[redacted]");
            }
        }
        text = text.replace(secret, "[redacted]");
    }
    if text.len() > OUTPUT_BYTES {
        let end = (0..=OUTPUT_BYTES)
            .rev()
            .find(|index| text.is_char_boundary(*index))
            .unwrap_or(0);
        text.truncate(end);
        truncated = true;
    }
    QaOutput { text, truncated }
}
