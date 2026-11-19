//! Opens a pull request through the GitHub CLI, with the user's own login.

use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const GH_TIME_LIMIT: Duration = Duration::from_secs(60);

/// Why a pull request could not be opened.
#[derive(Debug, thiserror::Error)]
#[must_use]
pub enum PullRequestError {
    /// `gh` could not be started, usually because it is not installed.
    #[error("could not run gh, is the GitHub CLI installed and signed in? {0}")]
    Spawn(#[from] io::Error),
    /// `gh` ran and refused.
    #[error("gh could not create the pull request: {0}")]
    Failed(String),
}

/// What the pull request says and which branches it joins.
#[derive(Debug, Clone, Copy)]
pub struct PullRequest<'a> {
    /// Branch the changes should land on.
    pub base: &'a str,
    /// Branch that holds the changes. It must already be pushed.
    pub head: &'a str,
    /// One-line title.
    pub title: &'a str,
    /// Description; may be empty.
    pub body: &'a str,
}

/// Opens the pull request from inside `worktree` and returns its URL.
///
/// # Errors
/// Fails when `gh` cannot run or exits with a failure status, for example when the branch is
/// not pushed or a pull request already exists.
pub fn create_pull_request(
    worktree: &Path,
    request: &PullRequest<'_>,
) -> Result<String, PullRequestError> {
    let output = brachio_process::output_within(
        Command::new("gh")
            .current_dir(worktree)
            .args(arguments(request)),
        GH_TIME_LIMIT,
    )?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(PullRequestError::Failed(stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn arguments(request: &PullRequest<'_>) -> Vec<String> {
    let mut arguments: Vec<String> = [
        "pr",
        "create",
        "--base",
        request.base,
        "--head",
        request.head,
    ]
    .map(str::to_owned)
    .to_vec();
    arguments.extend(["--title".to_owned(), request.title.to_owned()]);
    arguments.extend(["--body".to_owned(), request.body.to_owned()]);
    arguments
}

#[cfg(test)]
mod tests {
    use super::{arguments, PullRequest};

    #[test]
    fn every_value_is_one_argument_so_nothing_is_parsed_as_a_flag_or_a_shell_string() {
        let request = PullRequest {
            base: "main",
            head: "brachio/fix-1a2b3c",
            title: "--draft; rm -rf /",
            body: "line one\nline two",
        };
        assert_eq!(
            arguments(&request),
            [
                "pr",
                "create",
                "--base",
                "main",
                "--head",
                "brachio/fix-1a2b3c",
                "--title",
                "--draft; rm -rf /",
                "--body",
                "line one\nline two"
            ]
        );
    }
}
