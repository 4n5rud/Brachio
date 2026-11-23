//! Remembers each task's last known pull request status and decides when to ask GitHub again.
//! Read only: nothing here changes a pull request.
//!
//! The latest answer of each task is also saved in the store, so after a restart, or while GitHub
//! cannot be reached, the last known status is still shown, marked as out of date.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use brachio_git::{
    backoff_seconds, fetch_pull_request, fetch_pull_request_by_number, local_head,
    parse_pull_request_reference, repository_of, ChecksOverall, PullRequestSnapshot,
    PullRequestState, StatusError,
};
use brachio_store::{
    add_linked_pr, forget_pr_status, linked_prs, load_pr_status, record_activity, remove_linked_pr,
    save_pr_status, LinkedPr, RepoRecord, TaskRecord,
};
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
    /// Pull requests the user linked besides the branch's own.
    pub linked: Vec<LinkedPrReport>,
}

/// One linked pull request and what is known about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedPrReport {
    /// `owner/name`.
    pub repository: String,
    /// The pull request number.
    pub number: u64,
    /// The last answer, kept while GitHub cannot be reached.
    pub snapshot: Option<PullRequestSnapshot>,
    /// When that answer was read, in milliseconds since the Unix epoch.
    pub checked_at_ms: Option<i64>,
    /// Why the latest attempt failed.
    pub error: Option<String>,
}

#[derive(Default)]
struct LinkedEntry {
    snapshot: Option<PullRequestSnapshot>,
    checked_at_ms: Option<i64>,
    error: Option<String>,
    attempted_ms: i64,
}

/// An open linked pull request is looked at again after this long, unless the user insists.
const LINKED_REFRESH_MS: i64 = 30_000;

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
    linked: HashMap<(String, String, u64), LinkedEntry>,
    linked_in_flight: HashSet<String>,
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

    /// The linked pull requests that need GitHub now, or none while another lookup of the task's
    /// links runs. Merged and closed ones are read once; a failed one waits like an open one.
    pub fn begin_linked(
        &mut self,
        task: &str,
        links: &[LinkedPr],
        now_ms: i64,
        force: bool,
    ) -> Vec<LinkedPr> {
        if self.linked_in_flight.contains(task) {
            return Vec::new();
        }
        let due: Vec<LinkedPr> = links
            .iter()
            .filter(|link| {
                let Some(entry) =
                    self.linked
                        .get(&(task.to_owned(), link.repository.clone(), link.number))
                else {
                    return true;
                };
                let finished = entry
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.state != PullRequestState::Open);
                if finished && !force {
                    return false;
                }
                force || now_ms - entry.attempted_ms >= LINKED_REFRESH_MS
            })
            .cloned()
            .collect();
        if !due.is_empty() {
            self.linked_in_flight.insert(task.to_owned());
        }
        due
    }

    /// Records the answers for what `begin_linked` returned. A failure keeps the last answer.
    pub fn finish_linked(
        &mut self,
        task: &str,
        now_ms: i64,
        outcomes: Vec<(LinkedPr, Result<PullRequestSnapshot, StatusError>)>,
    ) {
        self.linked_in_flight.remove(task);
        for (link, outcome) in outcomes {
            let entry = self
                .linked
                .entry((task.to_owned(), link.repository, link.number))
                .or_default();
            entry.attempted_ms = now_ms;
            match outcome {
                Ok(snapshot) => {
                    entry.snapshot = Some(snapshot);
                    entry.checked_at_ms = Some(now_ms);
                    entry.error = None;
                }
                Err(error) => entry.error = Some(error.to_string()),
            }
        }
    }

    /// Forgets a link that was removed.
    pub fn forget_linked(&mut self, task: &str, repository: &str, number: u64) {
        self.linked
            .remove(&(task.to_owned(), repository.to_owned(), number));
    }

    /// What is known about each link, hiding the one that is the branch's own pull request.
    #[must_use]
    pub fn linked_report(&self, task: &str, links: &[LinkedPr]) -> Vec<LinkedPrReport> {
        let own = self
            .entries
            .get(task)
            .and_then(|entry| entry.snapshot.as_ref())
            .map(|snapshot| (snapshot.repository.clone(), snapshot.number));
        links
            .iter()
            .filter(|link| own.as_ref() != Some(&(link.repository.clone(), link.number)))
            .map(|link| {
                let entry =
                    self.linked
                        .get(&(task.to_owned(), link.repository.clone(), link.number));
                LinkedPrReport {
                    repository: link.repository.clone(),
                    number: link.number,
                    snapshot: entry.and_then(|entry| entry.snapshot.clone()),
                    checked_at_ms: entry.and_then(|entry| entry.checked_at_ms),
                    error: entry.and_then(|entry| entry.error.clone()),
                }
            })
            .collect()
    }

    /// Starts a task from an answer saved earlier. Does nothing when the task already has one.
    pub fn restore(&mut self, task: &str, snapshot: PullRequestSnapshot, checked_at_ms: i64) {
        self.entries
            .entry(task.to_owned())
            .or_insert_with(|| Entry {
                answered: true,
                snapshot: Some(snapshot),
                checked_at_ms: Some(checked_at_ms),
                ..Entry::default()
            });
    }

    /// Whether anything is known about the task in this run.
    #[must_use]
    pub fn knows(&self, task: &str) -> bool {
        self.entries.contains_key(task)
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
            linked: Vec::new(),
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

pub(crate) fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

/// Loads the answer saved before the last restart, once per task and run.
fn restore_saved(database: &Database, statuses: &PrStatuses, id: &str) -> Result<(), String> {
    if statuses.tracker()?.knows(id) {
        return Ok(());
    }
    let saved = load_pr_status(&*database.lock()?, id).map_err(|error| error.to_string())?;
    if let Some((json, checked_at_ms)) = saved {
        // An unreadable saved answer is dropped; the next lookup replaces it.
        if let Ok(snapshot) = serde_json::from_str(&json) {
            statuses.tracker()?.restore(id, snapshot, checked_at_ms);
        }
    }
    Ok(())
}

fn forget_snapshot(database: &Database, id: &str) {
    let forgotten = database.lock().and_then(|connection| {
        forget_pr_status(&connection, id).map_err(|error| error.to_string())
    });
    if let Err(error) = forgotten {
        tracing::warn!(%error, "could not drop the saved pull request status");
    }
}

fn save_snapshot(database: &Database, id: &str, snapshot: &PullRequestSnapshot, at: i64) {
    let Ok(json) = serde_json::to_string(snapshot) else {
        return;
    };
    let saved = database.lock().and_then(|connection| {
        save_pr_status(&connection, id, &json, at).map_err(|error| error.to_string())
    });
    if let Err(error) = saved {
        tracing::warn!(%error, "could not save the pull request status");
    }
}

/// What changed between two answers, as `<event> #<number>` lines for the task's timeline.
fn pull_request_events(
    prior: Option<&PullRequestSnapshot>,
    current: &PullRequestSnapshot,
) -> Vec<String> {
    let number = current.number;
    let Some(prior) = prior.filter(|prior| prior.number == number) else {
        return vec![format!("found #{number}")];
    };
    let mut events = Vec::new();
    if prior.state != current.state {
        let event = match current.state {
            PullRequestState::Open => "reopened",
            PullRequestState::Merged => "merged",
            PullRequestState::Closed => "closed",
        };
        events.push(format!("{event} #{number}"));
    }
    if prior.checks.overall != current.checks.overall {
        match current.checks.overall {
            ChecksOverall::Success => events.push(format!("checks_passed #{number}")),
            ChecksOverall::Failure => events.push(format!("checks_failed #{number}")),
            ChecksOverall::Pending | ChecksOverall::None => {}
        }
    }
    events
}

fn record_pull_request_events(
    database: &Database,
    id: &str,
    prior: Option<&PullRequestSnapshot>,
    current: &PullRequestSnapshot,
    at: i64,
) {
    let Ok(connection) = database.lock() else {
        return;
    };
    for event in pull_request_events(prior, current) {
        if let Err(error) = record_activity(&connection, id, "pr_event", Some(&event), at) {
            tracing::warn!(%error, "could not record a pull request event");
        }
    }
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
    let is_git = RepoRecord::get(&*database.lock()?, &task.repo_id)
        .map_err(|error| error.to_string())?
        .is_git;
    if !is_git {
        // A plain folder has no branch, so there is no pull request to look for.
        return Ok(PrStatusReport {
            no_pull_request: true,
            ..PrStatusReport::default()
        });
    }
    restore_saved(&database, &statuses, &id)?;
    let prior = statuses.tracker()?.report(&id, now_ms()).snapshot;
    let links = linked_prs(&*database.lock()?, &id).map_err(|error| error.to_string())?;
    let decision = statuses.tracker()?.begin(&id, now_ms(), force);
    if decision == Decision::Fetch {
        let worktree = task.worktree_path.clone();
        let branch = task.branch.clone();
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            let repository = repository_of(&worktree)?;
            fetch_pull_request(&worktree, &repository, &branch)
        })
        .await
        // A lookup that panicked must still be finished, or the task stays "being looked up".
        .unwrap_or_else(|error| {
            Err(StatusError::Failed {
                message: error.to_string(),
                rate_limited: false,
            })
        });
        let at = now_ms();
        match &outcome {
            Ok(Some(snapshot)) => {
                save_snapshot(&database, &id, snapshot, at);
                record_pull_request_events(&database, &id, prior.as_ref(), snapshot, at);
            }
            // The pull request is gone, so the saved one must not come back after a restart.
            Ok(None) => forget_snapshot(&database, &id),
            Err(_) => {}
        }
        statuses.tracker()?.finish(&id, at, outcome);
    }
    let due = statuses
        .tracker()?
        .begin_linked(&id, &links, now_ms(), force);
    if !due.is_empty() {
        let worktree = task.worktree_path.clone();
        let outcomes = tauri::async_runtime::spawn_blocking(move || {
            due.into_iter()
                .map(|link| {
                    let outcome =
                        fetch_pull_request_by_number(&worktree, &link.repository, link.number);
                    (link, outcome)
                })
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        statuses.tracker()?.finish_linked(&id, now_ms(), outcomes);
    }
    let mut report = statuses.tracker()?.report(&id, now_ms());
    report.linked = statuses.tracker()?.linked_report(&id, &links);
    report.local_head = local_head(&task.worktree_path).ok();
    report.head_matches_local = report
        .snapshot
        .as_ref()
        .zip(report.local_head.as_ref())
        .map(|(snapshot, local)| &snapshot.head_sha == local);
    Ok(report)
}

/// Links another pull request to the task, by number or GitHub address. It is looked up first,
/// so a number that does not exist is refused instead of kept.
#[tauri::command]
pub async fn task_link_pr(
    database: State<'_, Database>,
    statuses: State<'_, PrStatuses>,
    id: String,
    reference: String,
) -> Result<(), String> {
    let parsed = parse_pull_request_reference(&reference)
        .ok_or("Enter a pull request number or a github.com pull request address.")?;
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(|error| error.to_string())?;
    let worktree = task.worktree_path.clone();
    let (repository, snapshot) = tauri::async_runtime::spawn_blocking(move || {
        let repository = match parsed.repository {
            Some(repository) => repository,
            None => repository_of(&worktree)?,
        };
        let snapshot = fetch_pull_request_by_number(&worktree, &repository, parsed.number)?;
        Ok::<_, StatusError>((repository, snapshot))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    let at = now_ms();
    add_linked_pr(&*database.lock()?, &id, &repository, snapshot.number, at)
        .map_err(|error| error.to_string())?;
    let link = LinkedPr {
        repository,
        number: snapshot.number,
    };
    statuses
        .tracker()?
        .finish_linked(&id, at, vec![(link, Ok(snapshot))]);
    Ok(())
}

/// Stops showing a linked pull request on the task.
#[tauri::command]
pub fn task_unlink_pr(
    database: State<'_, Database>,
    statuses: State<'_, PrStatuses>,
    id: String,
    repository: String,
    number: u64,
) -> Result<(), String> {
    remove_linked_pr(&*database.lock()?, &id, &repository, number)
        .map_err(|error| error.to_string())?;
    statuses.tracker()?.forget_linked(&id, &repository, number);
    Ok(())
}

#[cfg(test)]
mod tests {
    use brachio_git::{
        ChecksOverall, ChecksSummary, Conflict, PullRequestSnapshot, PullRequestState, ReviewState,
        StatusError,
    };

    use super::{pull_request_events, Decision, Tracker};

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
    fn a_saved_answer_is_shown_when_the_first_lookup_after_a_restart_fails() {
        let mut tracker = Tracker::default();
        assert!(!tracker.knows("t"));
        tracker.restore("t", snapshot(PullRequestState::Open, "ccc"), 5_000);
        assert!(tracker.knows("t"));
        assert_eq!(tracker.begin("t", 100_000, false), Decision::Fetch);
        tracker.finish("t", 100_000, Err(offline()));

        let report = tracker.report("t", 100_000);
        assert_eq!(report.snapshot.expect("restored").head_sha, "ccc");
        assert_eq!(report.checked_at_ms, Some(5_000));
        assert!(report.error.is_some());
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

    fn link(number: u64) -> brachio_store::LinkedPr {
        brachio_store::LinkedPr {
            repository: "o/r".to_owned(),
            number,
        }
    }

    #[test]
    fn a_linked_pull_request_is_read_once_when_merged_and_kept_when_github_fails() {
        let mut tracker = Tracker::default();
        let links = [link(8), link(9)];
        assert_eq!(tracker.begin_linked("t", &links, 0, false).len(), 2);
        assert!(tracker.begin_linked("t", &links, 0, true).is_empty());
        tracker.finish_linked(
            "t",
            0,
            vec![
                (link(8), Ok(snapshot(PullRequestState::Merged, "m"))),
                (link(9), Ok(snapshot(PullRequestState::Open, "o"))),
            ],
        );
        assert!(tracker.begin_linked("t", &links, 10_000, false).is_empty());
        assert_eq!(tracker.begin_linked("t", &links, 40_000, false), [link(9)]);
        tracker.finish_linked("t", 40_000, vec![(link(9), Err(offline()))]);
        let report = tracker.linked_report("t", &links);
        assert_eq!(
            report[0].snapshot.as_ref().map(|s| s.state),
            Some(PullRequestState::Merged)
        );
        assert_eq!(
            report[1].snapshot.as_ref().map(|s| s.head_sha.as_str()),
            Some("o")
        );
        assert_eq!(report[1].error.as_deref(), Some("could not resolve host"));
        assert_eq!(
            tracker.begin_linked("t", &[link(8)], 99_000, true),
            [link(8)]
        );
    }

    #[test]
    fn the_branchs_own_pull_request_is_not_listed_twice() {
        let mut tracker = Tracker::default();
        tracker.begin("t", 0, false);
        tracker.finish("t", 0, Ok(Some(snapshot(PullRequestState::Open, "a"))));
        let listed = tracker.linked_report("t", &[link(7), link(8)]);
        assert_eq!(listed.iter().map(|l| l.number).collect::<Vec<_>>(), [8]);
    }

    #[test]
    fn timeline_events_name_what_changed_in_the_pull_request() {
        let open = snapshot(PullRequestState::Open, "a");
        assert_eq!(pull_request_events(None, &open), ["found #7"]);

        let mut failing = open.clone();
        failing.checks.overall = ChecksOverall::Failure;
        assert_eq!(
            pull_request_events(Some(&open), &failing),
            ["checks_failed #7"]
        );
        assert_eq!(
            pull_request_events(Some(&failing), &open),
            ["checks_passed #7"]
        );

        let merged = snapshot(PullRequestState::Merged, "a");
        assert_eq!(pull_request_events(Some(&open), &merged), ["merged #7"]);
        assert_eq!(pull_request_events(Some(&merged), &open), ["reopened #7"]);
        assert!(pull_request_events(Some(&open), &open).is_empty());
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
