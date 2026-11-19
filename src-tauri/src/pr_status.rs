//! Remembers each task's last known pull request status and decides when to ask GitHub again.
//! Read only: nothing here changes a pull request.
//!
//! The first version keeps the answers in memory. After a restart the first lookup fills them in
//! again; saving them across restarts needs a table in the store and is left for a later change.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use brachio_git::{
    backoff_seconds, fetch_pull_request, local_head, repository_of, PullRequestSnapshot,
    PullRequestState, StatusError,
};
use brachio_store::TaskRecord;
use serde::Serialize;
use tauri::State;

use crate::workspace::Database;

/// What the Inspector shows for one task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrStatusReport {
    /// The last pull request GitHub described, or none.
    pub snapshot: Option<PullRequestSnapshot>,
    /// When that answer was read, in milliseconds since the Unix epoch.
    pub checked_at_ms: Option<i64>,
    /// GitHub was reached and the branch has no pull request.
    pub no_pull_request: bool,
    /// Why the latest attempt failed. While this is set the snapshot, if any, is out of date.
    pub error: Option<String>,
    /// Seconds until another attempt is made after a failure.
    pub retry_in_seconds: Option<u64>,
    /// The commit checked out in the worktree.
    pub local_head: Option<String>,
    /// Whether the worktree is at the commit the checks belong to.
    pub head_matches_local: Option<bool>,
}

#[derive(Default)]
struct Entry {
    answered: bool,
    snapshot: Option<PullRequestSnapshot>,
    checked_at_ms: Option<i64>,
    error: Option<String>,
    failures: u32,
    retry_after_ms: i64,
    rate_limited: bool,
    in_flight: bool,
}

/// What to do with a request for a task's status.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Ask GitHub.
    Fetch,
    /// Answer with what is known.
    UseKnown,
}

/// The status of every task looked at so far.
#[derive(Default)]
pub struct Tracker {
    entries: HashMap<String, Entry>,
}

impl Tracker {
    /// Decides whether a request needs GitHub. Pull requests that are merged or closed are never
    /// asked about again unless `force`; a wait after a failure is kept unless the user insists,
    /// except after a rate limit, which nobody can talk their way around.
    pub fn begin(&mut self, task: &str, now_ms: i64, force: bool) -> Decision {
        let entry = self.entries.entry(task.to_owned()).or_default();
        if entry.in_flight {
            return Decision::UseKnown;
        }
        let finished = entry
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.state != PullRequestState::Open);
        if finished && !force {
            return Decision::UseKnown;
        }
        let waiting = now_ms < entry.retry_after_ms;
        if waiting && (!force || entry.rate_limited) {
            return Decision::UseKnown;
        }
        entry.in_flight = true;
        Decision::Fetch
    }

    /// Records the outcome of a lookup that `begin` allowed. A failure keeps the last good
    /// answer and starts a wait.
    pub fn finish(
        &mut self,
        task: &str,
        now_ms: i64,
        outcome: Result<Option<PullRequestSnapshot>, StatusError>,
    ) {
        let entry = self.entries.entry(task.to_owned()).or_default();
        entry.in_flight = false;
        match outcome {
            Ok(snapshot) => {
                entry.answered = true;
                entry.snapshot = snapshot;
                entry.checked_at_ms = Some(now_ms);
                entry.error = None;
                entry.failures = 0;
                entry.retry_after_ms = 0;
                entry.rate_limited = false;
            }
            Err(error) => {
                entry.failures = entry.failures.saturating_add(1);
                entry.rate_limited = error.is_rate_limited();
                let wait = backoff_seconds(entry.failures, entry.rate_limited);
                entry.retry_after_ms =
                    now_ms.saturating_add(i64::try_from(wait).unwrap_or(i64::MAX) * 1000);
                entry.error = Some(error.to_string());
            }
        }
    }

    /// What is known about a task right now.
    #[must_use]
    pub fn report(&self, task: &str, now_ms: i64) -> PrStatusReport {
        let Some(entry) = self.entries.get(task) else {
            return PrStatusReport::default();
        };
        let waiting_ms = (entry.retry_after_ms - now_ms).max(0);
        PrStatusReport {
            snapshot: entry.snapshot.clone(),
            checked_at_ms: entry.checked_at_ms,
            no_pull_request: entry.answered && entry.snapshot.is_none() && entry.error.is_none(),
            error: entry.error.clone(),
            retry_in_seconds: (entry.error.is_some() && waiting_ms > 0)
                .then(|| u64::try_from((waiting_ms + 999) / 1000).unwrap_or(0)),
            local_head: None,
            head_matches_local: None,
        }
    }
}

/// App state: the tracker behind a lock.
#[derive(Default)]
pub struct PrStatuses(Mutex<Tracker>);

impl PrStatuses {
    fn tracker(&self) -> Result<std::sync::MutexGuard<'_, Tracker>, String> {
        self.0
            .lock()
            .map_err(|_| "pull request status lock poisoned".to_owned())
    }
}

fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

/// Reads where the task's pull request stands. `force` is the user pressing Refresh.
#[tauri::command]
pub async fn task_pr_status(
    database: State<'_, Database>,
    statuses: State<'_, PrStatuses>,
    id: String,
    force: bool,
) -> Result<PrStatusReport, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(|error| error.to_string())?;
    let decision = statuses.tracker()?.begin(&id, now_ms(), force);
    if decision == Decision::Fetch {
        let worktree = task.worktree_path.clone();
        let branch = task.branch.clone();
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            let repository = repository_of(&worktree)?;
            fetch_pull_request(&worktree, &repository, &branch)
        })
        .await
        .map_err(|error| error.to_string())?;
        statuses.tracker()?.finish(&id, now_ms(), outcome);
    }
    let mut report = statuses.tracker()?.report(&id, now_ms());
    report.local_head = local_head(&task.worktree_path).ok();
    report.head_matches_local = report
        .snapshot
        .as_ref()
        .zip(report.local_head.as_ref())
        .map(|(snapshot, local)| &snapshot.head_sha == local);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use brachio_git::{
        ChecksOverall, ChecksSummary, Conflict, PullRequestSnapshot, PullRequestState, ReviewState,
        StatusError,
    };

    use super::{Decision, Tracker};

    fn snapshot(state: PullRequestState, head: &str) -> PullRequestSnapshot {
        PullRequestSnapshot {
            repository: "o/r".to_owned(),
            number: 7,
            url: "https://github.com/o/r/pull/7".to_owned(),
            state,
            draft: false,
            base_branch: "main".to_owned(),
            head_branch: "feat/x".to_owned(),
            head_sha: head.to_owned(),
            checks: ChecksSummary {
                overall: ChecksOverall::Success,
                passing: 1,
                failing: 0,
                pending: 0,
                skipped: 0,
            },
            failed_checks: Vec::new(),
            review: ReviewState::NotRequired,
            conflict: Conflict::Clean,
        }
    }

    fn offline() -> StatusError {
        StatusError::Failed {
            message: "could not resolve host".to_owned(),
            rate_limited: false,
        }
    }

    fn rate_limited() -> StatusError {
        StatusError::Failed {
            message: "API rate limit exceeded".to_owned(),
            rate_limited: true,
        }
    }

    #[test]
    fn going_offline_keeps_the_last_answer_with_its_time_and_the_reason() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.begin("t", 1_000, false), Decision::Fetch);
        tracker.finish(
            "t",
            1_000,
            Ok(Some(snapshot(PullRequestState::Open, "aaa"))),
        );
        assert_eq!(tracker.begin("t", 61_000, false), Decision::Fetch);
        tracker.finish("t", 61_000, Err(offline()));

        let report = tracker.report("t", 62_000);
        assert_eq!(report.snapshot.expect("kept").head_sha, "aaa");
        assert_eq!(report.checked_at_ms, Some(1_000));
        assert_eq!(report.error.as_deref(), Some("could not resolve host"));
        assert_eq!(report.retry_in_seconds, Some(29));
    }

    #[test]
    fn a_failure_makes_ordinary_refreshes_wait_but_the_user_can_insist() {
        let mut tracker = Tracker::default();
        tracker.begin("t", 0, false);
        tracker.finish("t", 0, Err(offline()));
        assert_eq!(tracker.begin("t", 10_000, false), Decision::UseKnown);
        assert_eq!(tracker.begin("t", 10_000, true), Decision::Fetch);
        tracker.finish("t", 10_000, Err(offline()));
        // Two failures in a row wait 60 seconds.
        assert_eq!(tracker.report("t", 10_000).retry_in_seconds, Some(60));
    }

    #[test]
    fn a_rate_limit_is_not_overridden_by_pressing_refresh() {
        let mut tracker = Tracker::default();
        tracker.begin("t", 0, false);
        tracker.finish("t", 0, Err(rate_limited()));
        assert_eq!(tracker.begin("t", 60_000, true), Decision::UseKnown);
        assert_eq!(tracker.begin("t", 300_000, false), Decision::Fetch);
    }

    #[test]
    fn a_merged_pull_request_is_not_asked_about_again_unless_forced() {
        let mut tracker = Tracker::default();
        tracker.begin("t", 0, false);
        tracker.finish("t", 0, Ok(Some(snapshot(PullRequestState::Merged, "bbb"))));
        assert_eq!(tracker.begin("t", 3_600_000, false), Decision::UseKnown);
        assert_eq!(tracker.begin("t", 3_600_000, true), Decision::Fetch);
    }

    #[test]
    fn a_new_head_replaces_the_old_answer_instead_of_inheriting_its_checks() {
        let mut tracker = Tracker::default();
        tracker.begin("t", 0, false);
        tracker.finish("t", 0, Ok(Some(snapshot(PullRequestState::Open, "old"))));
        tracker.begin("t", 60_000, false);
        tracker.finish(
            "t",
            60_000,
            Ok(Some(snapshot(PullRequestState::Open, "new"))),
        );
        assert_eq!(
            tracker
                .report("t", 60_000)
                .snapshot
                .expect("answer")
                .head_sha,
            "new"
        );
    }

    #[test]
    fn two_requests_at_once_ask_github_only_once_and_a_branch_without_a_pull_request_is_remembered()
    {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.begin("t", 0, false), Decision::Fetch);
        assert_eq!(tracker.begin("t", 0, true), Decision::UseKnown);
        tracker.finish("t", 0, Ok(None));
        let report = tracker.report("t", 1);
        assert!(report.no_pull_request && report.error.is_none() && report.snapshot.is_none());
        assert!(!Tracker::default().report("never-asked", 1).no_pull_request);
    }
}
