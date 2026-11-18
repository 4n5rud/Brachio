//! Reads where a task's pull request stands, through the GitHub CLI and the user's own login.
//! Nothing here writes to GitHub.
//!
//! Every answer names the repository, the pull request number and the commit it describes, so a
//! result is never applied to another repository's pull request or to a newer commit.

use std::io;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::git::{Git, GitError};

/// Failing checks listed for one pull request, so a very large workflow cannot flood the screen.
const MAX_LISTED_CHECKS: usize = 20;
/// The first wait after a failed lookup, in seconds. Each further failure doubles it.
const FIRST_BACKOFF_SECONDS: u64 = 30;
/// The longest wait between lookups after failures.
const LONGEST_BACKOFF_SECONDS: u64 = 600;
/// The shortest wait after GitHub says the request limit is reached.
const RATE_LIMIT_BACKOFF_SECONDS: u64 = 300;

/// Why a lookup failed.
#[derive(Debug, thiserror::Error)]
#[must_use]
pub enum StatusError {
    /// `gh` could not be started, usually because it is not installed.
    #[error("could not run gh, is the GitHub CLI installed and signed in? {0}")]
    Spawn(#[from] io::Error),
    /// `gh` ran and refused, for example without a network connection.
    #[error("{message}")]
    Failed {
        /// What `gh` said.
        message: String,
        /// Whether GitHub reported that the request limit is reached.
        rate_limited: bool,
    },
    /// `gh` answered with something that is not the expected JSON.
    #[error("could not read the answer of gh: {0}")]
    Unreadable(String),
    /// git could not read the worktree.
    #[error(transparent)]
    Git(#[from] GitError),
}

impl StatusError {
    /// Whether waiting longer is the right response.
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        matches!(
            self,
            StatusError::Failed {
                rate_limited: true,
                ..
            }
        )
    }
}

/// Where a pull request is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestState {
    /// Still open: the only kind that is refreshed.
    Open,
    /// Merged: nothing more will change.
    Merged,
    /// Closed without merging.
    Closed,
}

/// What the checks of one commit add up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksOverall {
    /// At least one check failed.
    Failure,
    /// No check failed, and some are still waiting or running.
    Pending,
    /// Every check finished and none failed.
    Success,
    /// The commit has no checks.
    None,
}

/// The reviewers' verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// Approved.
    Approved,
    /// A reviewer asked for changes.
    ChangesRequested,
    /// A review is required and has not been given.
    ReviewRequired,
    /// The repository does not ask for reviews.
    NotRequired,
}

/// Whether the pull request can be merged without resolving conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Conflict {
    /// It merges cleanly.
    Clean,
    /// It conflicts with the base branch.
    Conflicting,
    /// GitHub has not worked it out yet, or the pull request is no longer open.
    Unknown,
}

/// The totals of a commit's checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecksSummary {
    /// The combined verdict.
    pub overall: ChecksOverall,
    /// Checks that finished well, including neutral ones.
    pub passing: u32,
    /// Checks that failed, were cancelled or timed out.
    pub failing: u32,
    /// Checks that are queued or running.
    pub pending: u32,
    /// Checks that were skipped.
    pub skipped: u32,
}

/// A failed check with the address of its original log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FailedCheck {
    /// The check's name.
    pub name: String,
    /// Where GitHub shows its log, when GitHub gave one.
    pub url: Option<String>,
}

/// A pull request as GitHub described it at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSnapshot {
    /// `owner/name`.
    pub repository: String,
    /// The pull request number in that repository.
    pub number: u64,
    /// Its address.
    pub url: String,
    /// Open, merged or closed.
    pub state: PullRequestState,
    /// Whether it is a draft.
    pub draft: bool,
    /// The branch it merges into.
    pub base_branch: String,
    /// The branch it comes from.
    pub head_branch: String,
    /// The commit the checks and the conflict state below belong to.
    pub head_sha: String,
    /// Totals of the checks of that commit.
    pub checks: ChecksSummary,
    /// The failed checks, at most twenty.
    pub failed_checks: Vec<FailedCheck>,
    /// The reviewers' verdict.
    pub review: ReviewState,
    /// Whether it can be merged cleanly.
    pub conflict: Conflict,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPullRequest {
    number: u64,
    url: String,
    state: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    base_ref_name: String,
    #[serde(default)]
    head_ref_name: String,
    #[serde(default)]
    head_ref_oid: String,
    #[serde(default)]
    mergeable: String,
    #[serde(default)]
    review_decision: String,
    #[serde(default)]
    status_check_rollup: Vec<RawCheck>,
    #[serde(default)]
    updated_at: String,
}

/// Either a check run or a commit status, which GitHub lists together.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCheck {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    details_url: Option<String>,
    #[serde(default)]
    target_url: Option<String>,
}

enum Verdict {
    Passing,
    Skipped,
    Failing,
    Pending,
}

impl RawCheck {
    fn verdict(&self) -> Verdict {
        if let Some(state) = &self.state {
            return match state.as_str() {
                "SUCCESS" => Verdict::Passing,
                "FAILURE" | "ERROR" => Verdict::Failing,
                _ => Verdict::Pending,
            };
        }
        if self.status.as_deref() != Some("COMPLETED") {
            return Verdict::Pending;
        }
        match self.conclusion.as_deref().unwrap_or_default() {
            "SUCCESS" | "NEUTRAL" => Verdict::Passing,
            "SKIPPED" => Verdict::Skipped,
            _ => Verdict::Failing,
        }
    }

    fn label(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.context.clone())
            .unwrap_or_else(|| "check".to_owned())
    }
}

/// Adds up a commit's checks and lists the failures.
fn summarize_checks(checks: &[RawCheck]) -> (ChecksSummary, Vec<FailedCheck>) {
    let (mut passing, mut failing, mut pending, mut skipped) = (0, 0, 0, 0);
    let mut failed = Vec::new();
    for check in checks {
        match check.verdict() {
            Verdict::Passing => passing += 1,
            Verdict::Skipped => skipped += 1,
            Verdict::Pending => pending += 1,
            Verdict::Failing => {
                failing += 1;
                if failed.len() < MAX_LISTED_CHECKS {
                    failed.push(FailedCheck {
                        name: check.label(),
                        url: check
                            .details_url
                            .clone()
                            .or_else(|| check.target_url.clone()),
                    });
                }
            }
        }
    }
    let overall = if failing > 0 {
        ChecksOverall::Failure
    } else if pending > 0 {
        ChecksOverall::Pending
    } else if passing + skipped > 0 {
        ChecksOverall::Success
    } else {
        ChecksOverall::None
    };
    (
        ChecksSummary {
            overall,
            passing,
            failing,
            pending,
            skipped,
        },
        failed,
    )
}

fn snapshot_of(repository: &str, raw: &RawPullRequest) -> PullRequestSnapshot {
    let (checks, failed_checks) = summarize_checks(&raw.status_check_rollup);
    let state = match raw.state.as_str() {
        "OPEN" => PullRequestState::Open,
        "MERGED" => PullRequestState::Merged,
        _ => PullRequestState::Closed,
    };
    PullRequestSnapshot {
        repository: repository.to_owned(),
        number: raw.number,
        url: raw.url.clone(),
        state,
        draft: raw.is_draft,
        base_branch: raw.base_ref_name.clone(),
        head_branch: raw.head_ref_name.clone(),
        head_sha: raw.head_ref_oid.clone(),
        checks,
        failed_checks,
        review: match raw.review_decision.as_str() {
            "APPROVED" => ReviewState::Approved,
            "CHANGES_REQUESTED" => ReviewState::ChangesRequested,
            "REVIEW_REQUIRED" => ReviewState::ReviewRequired,
            _ => ReviewState::NotRequired,
        },
        conflict: match raw.mergeable.as_str() {
            "MERGEABLE" => Conflict::Clean,
            "CONFLICTING" => Conflict::Conflicting,
            _ => Conflict::Unknown,
        },
    }
}

/// True when `url` is a pull request address inside `repository`. GitHub is asked about one
/// repository, and this keeps an answer about another one from ever being used.
fn belongs_to(repository: &str, url: &str) -> bool {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(_, path)| path.starts_with(&format!("{repository}/pull/")))
}

/// Picks the pull request that describes the branch: an open one first, then the most recently
/// updated one. Pull requests of other repositories are ignored.
///
/// # Errors
/// Fails when `json` is not the list `gh pr list --json` prints.
pub fn parse_pull_requests(
    repository: &str,
    json: &str,
) -> Result<Option<PullRequestSnapshot>, StatusError> {
    let mut found: Vec<RawPullRequest> =
        serde_json::from_str(json).map_err(|error| StatusError::Unreadable(error.to_string()))?;
    found.retain(|raw| belongs_to(repository, &raw.url));
    found.sort_by(|a, b| {
        let open = |raw: &RawPullRequest| u8::from(raw.state != "OPEN");
        open(a)
            .cmp(&open(b))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    Ok(found.first().map(|raw| snapshot_of(repository, raw)))
}

/// How long to wait before the next lookup after `failures` failures in a row.
#[must_use]
pub fn backoff_seconds(failures: u32, rate_limited: bool) -> u64 {
    let doubled = FIRST_BACKOFF_SECONDS
        .checked_shl(failures.saturating_sub(1))
        .unwrap_or(LONGEST_BACKOFF_SECONDS)
        .min(LONGEST_BACKOFF_SECONDS);
    if rate_limited {
        doubled.max(RATE_LIMIT_BACKOFF_SECONDS)
    } else {
        doubled
    }
}

fn run_gh(worktree: &Path, args: &[&str]) -> Result<String, StatusError> {
    let output = Command::new("gh")
        .current_dir(worktree)
        .args(args)
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let rate_limited = message.to_lowercase().contains("rate limit");
        return Err(StatusError::Failed {
            message,
            rate_limited,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The `owner/name` of the repository the worktree's remote points at.
///
/// # Errors
/// Fails when `gh` cannot tell, for example without a GitHub remote or a login.
pub fn repository_of(worktree: &Path) -> Result<String, StatusError> {
    let answer = run_gh(
        worktree,
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ],
    )?;
    Ok(answer.trim().to_owned())
}

/// Reads the pull request of `branch` in `repository`. `None` when the branch has none.
///
/// # Errors
/// Fails when `gh` cannot be run, is refused, or answers with something unreadable.
pub fn fetch_pull_request(
    worktree: &Path,
    repository: &str,
    branch: &str,
) -> Result<Option<PullRequestSnapshot>, StatusError> {
    let json = run_gh(
        worktree,
        &[
            "pr",
            "list",
            "--repo",
            repository,
            "--head",
            branch,
            "--state",
            "all",
            "--limit",
            "10",
            "--json",
            "number,url,state,isDraft,baseRefName,headRefName,headRefOid,mergeable,reviewDecision,statusCheckRollup,updatedAt",
        ],
    )?;
    parse_pull_requests(repository, &json)
}

/// The commit checked out in the worktree, to compare with the pull request's head.
///
/// # Errors
/// Fails when git cannot read the worktree.
pub fn local_head(worktree: &Path) -> Result<String, StatusError> {
    Ok(Git::new(worktree).run(&["rev-parse", "HEAD"])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like `gh pr list --json` output: one pull request of this repository.
    const FAILED_CI: &str = r#"[{"baseRefName":"main","headRefName":"fix/explain-non-git-folder","headRefOid":"68395383847f768023545324589198088c4971e4","isDraft":false,"mergeable":"MERGEABLE","number":65,"reviewDecision":"","state":"OPEN","updatedAt":"2026-11-18T15:35:28Z","url":"https://github.com/4n5rud/Brachio/pull/65","statusCheckRollup":[
        {"__typename":"CheckRun","completedAt":"2026-11-18T15:35:58Z","conclusion":"FAILURE","detailsUrl":"https://github.com/4n5rud/Brachio/actions/runs/44337487778/job/1","name":"Rust (windows-latest)","startedAt":"2026-11-18T15:35:56Z","status":"COMPLETED","workflowName":"CI"},
        {"__typename":"CheckRun","conclusion":"SKIPPED","detailsUrl":"https://github.com/4n5rud/Brachio/actions/runs/2/job/2","name":"issue","status":"COMPLETED","workflowName":"Labels"},
        {"__typename":"CheckRun","conclusion":"SUCCESS","detailsUrl":"https://github.com/4n5rud/Brachio/actions/runs/3/job/3","name":"UI","status":"COMPLETED","workflowName":"CI"},
        {"__typename":"CheckRun","conclusion":"","detailsUrl":"https://github.com/4n5rud/Brachio/actions/runs/4/job/4","name":"Version files agree","status":"IN_PROGRESS","workflowName":"CI"},
        {"__typename":"StatusContext","context":"ci/legacy","state":"ERROR","targetUrl":"https://ci.example/build/9"}
    ]}]"#;

    fn parse(json: &str) -> Option<PullRequestSnapshot> {
        parse_pull_requests("4n5rud/Brachio", json).expect("readable")
    }

    #[test]
    fn a_failing_commit_is_told_apart_from_running_skipped_and_passing_checks() {
        let snapshot = parse(FAILED_CI).expect("a pull request");
        assert_eq!(
            (
                snapshot.repository.as_str(),
                snapshot.number,
                snapshot.head_sha.as_str()
            ),
            (
                "4n5rud/Brachio",
                65,
                "68395383847f768023545324589198088c4971e4"
            )
        );
        assert_eq!(snapshot.checks.overall, ChecksOverall::Failure);
        assert_eq!(
            (
                snapshot.checks.passing,
                snapshot.checks.failing,
                snapshot.checks.pending,
                snapshot.checks.skipped
            ),
            (1, 2, 1, 1)
        );
        assert_eq!(snapshot.conflict, Conflict::Clean);
        assert_eq!(snapshot.review, ReviewState::NotRequired);
        assert_eq!(
            snapshot.failed_checks,
            [
                FailedCheck {
                    name: "Rust (windows-latest)".to_owned(),
                    url: Some(
                        "https://github.com/4n5rud/Brachio/actions/runs/44337487778/job/1"
                            .to_owned()
                    ),
                },
                FailedCheck {
                    name: "ci/legacy".to_owned(),
                    url: Some("https://ci.example/build/9".to_owned()),
                },
            ]
        );
    }

    #[test]
    fn checks_that_are_all_running_are_pending_and_a_commit_without_checks_is_none() {
        let running = r#"[{"number":7,"url":"https://github.com/o/r/pull/7","state":"OPEN","statusCheckRollup":[{"name":"build","status":"QUEUED","conclusion":""}]}]"#;
        let none = r#"[{"number":8,"url":"https://github.com/o/r/pull/8","state":"OPEN","statusCheckRollup":[]}]"#;
        let pending = parse_pull_requests("o/r", running)
            .expect("readable")
            .expect("one");
        let empty = parse_pull_requests("o/r", none)
            .expect("readable")
            .expect("one");
        assert_eq!(pending.checks.overall, ChecksOverall::Pending);
        assert_eq!(empty.checks.overall, ChecksOverall::None);
        assert_eq!(empty.conflict, Conflict::Unknown);
    }

    #[test]
    fn an_open_pull_request_wins_over_a_newer_merged_one_and_other_repositories_are_ignored() {
        let json = r#"[
            {"number":1,"url":"https://github.com/o/r/pull/1","state":"MERGED","updatedAt":"2026-11-19T00:00:00Z"},
            {"number":2,"url":"https://github.com/o/r/pull/2","state":"OPEN","updatedAt":"2026-11-01T00:00:00Z"},
            {"number":3,"url":"https://github.com/other/r/pull/3","state":"OPEN","updatedAt":"2026-11-20T00:00:00Z"}
        ]"#;
        let chosen = parse_pull_requests("o/r", json)
            .expect("readable")
            .expect("one");
        assert_eq!(chosen.number, 2);
        assert!(parse_pull_requests("nobody/else", json)
            .expect("readable")
            .is_none());
    }

    #[test]
    fn a_branch_without_a_pull_request_is_not_an_error() {
        assert!(parse("[]").is_none());
        assert!(parse_pull_requests("o/r", "not json").is_err());
    }

    #[test]
    fn waiting_doubles_with_each_failure_up_to_ten_minutes_and_rate_limits_wait_longer() {
        let waits: Vec<u64> = (1..=7).map(|n| backoff_seconds(n, false)).collect();
        assert_eq!(waits, [30, 60, 120, 240, 480, 600, 600]);
        assert_eq!(backoff_seconds(1, true), 300);
        assert_eq!(backoff_seconds(0, false), 30);
        assert_eq!(backoff_seconds(u32::MAX, false), 600);
    }
}
