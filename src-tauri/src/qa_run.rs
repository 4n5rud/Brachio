//! Runs a task's project QA command after the user approves it in the approval bar.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use brachio_qa::{
    failure_signature, FeedbackDecision, FeedbackGate, GatedOutcome, QaOutcome, QaReport,
};
use brachio_runtime::approval::Decision;
use brachio_store::{
    latest_qa_result, save_qa_result, NewQaResult, QaResultRecord, RepoRecord, TaskRecord,
};
use serde::Serialize;
use tauri::State;

use crate::hook_socket::Approvals;
use crate::pr_status;
use crate::workspace::Database;

/// Automatic fix attempts a task may use before a person has to look.
const MAX_AUTO_FIXES: u32 = 3;

/// The tasks running QA now and the feedback gate of each task. Results live in the database.
#[derive(Default)]
pub struct QaResults {
    gates: Mutex<HashMap<String, FeedbackGate>>,
    running: Mutex<HashSet<String>>,
}

/// A stored QA result plus what it means for the task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QaView {
    #[serde(flatten)]
    pub report: QaReport,
    /// Brachio verified this run on the commit a handoff of the task names.
    pub matches_handoff: bool,
    /// The feedback gate would allow another automatic attempt. Nothing is retried for you.
    pub retry_allowed: bool,
}

impl QaResults {
    fn retry_allowed(&self, task_id: &str, report: &QaReport) -> bool {
        let Ok(mut gates) = self.gates.lock() else {
            return false;
        };
        if report.outcome == QaOutcome::Passed {
            gates.remove(task_id);
            return true;
        }
        let first_line = [&report.stderr, &report.stdout]
            .into_iter()
            .flat_map(|text| text.lines())
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("");
        let signature = failure_signature(
            &format!("{}:{}", report.command, report.outcome.as_str()),
            first_line,
        );
        gates
            .entry(task_id.to_owned())
            .or_insert_with(|| FeedbackGate::new(MAX_AUTO_FIXES))
            .decide(signature)
            == FeedbackDecision::Send
    }
}

fn view(record: QaResultRecord) -> QaView {
    QaView {
        report: QaReport {
            command: record.command,
            directory: record.cwd,
            outcome: QaOutcome::parse(&record.outcome).unwrap_or(QaOutcome::ProcessError),
            exit_code: record.exit_code,
            duration_ms: u64::try_from(record.duration_ms).unwrap_or(0),
            sha: record.commit_sha,
            verified_by_brachio: record.verified_by_brachio,
            stdout: record.stdout,
            stderr: record.stderr,
            truncated: record.truncated,
        },
        matches_handoff: record.matches_handoff,
        retry_allowed: record.retry_allowed,
    }
}

/// Stores the result, attaches it to a handoff of the same commit, and returns what was stored.
fn store_report(
    database: &Database,
    results: &QaResults,
    task_id: &str,
    report: &QaReport,
    now_ms: i64,
) -> Result<QaView, String> {
    let retry_allowed = results.retry_allowed(task_id, report);
    let mut connection = database.lock()?;
    save_qa_result(
        &mut connection,
        &NewQaResult {
            task_id,
            command: &report.command,
            cwd: &report.directory,
            outcome: report.outcome.as_str(),
            exit_code: report.exit_code,
            duration_ms: i64::try_from(report.duration_ms).unwrap_or(i64::MAX),
            commit_sha: &report.sha,
            verified_by_brachio: report.verified_by_brachio,
            stdout: &report.stdout,
            stderr: &report.stderr,
            truncated: report.truncated,
            retry_allowed,
            created_at_ms: now_ms,
        },
    )
    .map_err(|error| error.to_string())?;
    read_latest(&connection, task_id)?.ok_or_else(|| "The QA result was not saved.".to_owned())
}

fn read_latest(connection: &rusqlite::Connection, task_id: &str) -> Result<Option<QaView>, String> {
    latest_qa_result(connection, task_id)
        .map(|record| record.map(view))
        .map_err(|error| error.to_string())
}

struct Running<'a>(&'a QaResults, String);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        if let Ok(mut running) = self.0.running.lock() {
            running.remove(&self.1);
        }
    }
}

/// Asks the user to approve the project's QA command, runs it once if they do, and returns the
/// result. `None` when the user declined. Nothing is retried or merged.
#[tauri::command]
pub async fn qa_run(
    database: State<'_, Database>,
    approvals: State<'_, Arc<Approvals>>,
    results: State<'_, Arc<QaResults>>,
    task_id: String,
) -> Result<Option<QaView>, String> {
    let (task, repo) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &task_id).map_err(|error| error.to_string())?;
        let repo =
            RepoRecord::get(&connection, &task.repo_id).map_err(|error| error.to_string())?;
        (task, repo)
    };
    if !repo.is_git {
        return Err("QA needs a git project.".to_owned());
    }
    let command = brachio_qa::read_command(&repo.path, &task.worktree_path)?
        .ok_or_else(|| "This project has no [qa] command in brachio.toml.".to_owned())?;
    let results = Arc::clone(&results);
    let shared = Arc::clone(&results);
    if !results
        .running
        .lock()
        .map_err(|_| "QA state lock poisoned".to_owned())?
        .insert(task_id.clone())
    {
        return Err("QA is already running for this task.".to_owned());
    }
    let approvals = Arc::clone(&approvals);
    let stored_id = task_id.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let _running = Running(&results, task_id.clone());
        let key = format!("{task_id}:agent");
        brachio_qa::run_after_approval(&command, &[], |review| {
            approvals.ask_blocking(key, review.to_owned()) == Some(Decision::Allow)
        })
    })
    .await
    .map_err(|error| error.to_string())??;
    match outcome {
        GatedOutcome::Ran(report) => {
            store_report(&database, &shared, &stored_id, &report, pr_status::now_ms()).map(Some)
        }
        GatedOutcome::Declined => Ok(None),
    }
}

/// The task's latest QA result, which survives restarts.
#[tauri::command]
pub fn qa_last_result(
    database: State<'_, Database>,
    task_id: String,
) -> Result<Option<QaView>, String> {
    read_latest(&*database.lock()?, &task_id)
}

#[cfg(test)]
mod tests {
    use crate::hook_socket::Notice;

    use super::*;

    fn database() -> Database {
        let connection = brachio_store::open_in_memory().expect("open");
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);",
            )
            .expect("seed");
        Database(Mutex::new(connection))
    }

    fn failed(stderr: &str) -> QaReport {
        QaReport {
            command: "npm test".into(),
            directory: "/w".into(),
            outcome: QaOutcome::Failed,
            exit_code: Some(1),
            duration_ms: 90,
            sha: "a".repeat(40),
            verified_by_brachio: true,
            stdout: String::new(),
            stderr: stderr.into(),
            truncated: false,
        }
    }

    #[test]
    fn the_stored_result_is_there_after_a_restart() {
        let database = database();
        let report = failed("boom");
        let stored =
            store_report(&database, &QaResults::default(), "t", &report, 5).expect("store");
        assert_eq!(stored.report, report);
        let restarted = QaResults::default();
        drop(restarted);
        let after = read_latest(&database.lock().expect("lock"), "t")
            .expect("read")
            .expect("a result");
        assert_eq!(after.report, report);
        assert!(!after.matches_handoff);
    }

    #[test]
    fn the_same_failure_twice_stops_further_automatic_retries() {
        let database = database();
        let results = QaResults::default();
        let first = store_report(&database, &results, "t", &failed("boom\n at a"), 1).expect("1");
        assert!(first.retry_allowed);
        let second = store_report(&database, &results, "t", &failed("boom\n at b"), 2).expect("2");
        assert!(!second.retry_allowed);
        let persisted = read_latest(&database.lock().expect("lock"), "t")
            .expect("read")
            .expect("a result");
        assert!(!persisted.retry_allowed);
    }

    #[test]
    fn a_run_on_the_commit_of_a_handoff_reports_the_match() {
        let database = database();
        {
            let connection = database.lock().expect("lock");
            brachio_store::record_handoff(
                &connection,
                &brachio_store::NewHandoff {
                    id: "h",
                    task_id: "t",
                    commit_sha: &"a".repeat(40),
                    base_sha: &"b".repeat(40),
                    summary: "done",
                    changed_paths: &[],
                    reported_tests: &[],
                    source: brachio_store::HandoffSource::ReportedByAgent,
                    created_at_ms: 1,
                },
            )
            .expect("handoff");
        }
        let stored =
            store_report(&database, &QaResults::default(), "t", &failed("x"), 3).expect("store");
        assert!(stored.matches_handoff);
    }

    #[test]
    fn an_app_question_waits_for_the_users_answer_and_closes_afterwards() {
        let notices = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&notices);
        let approvals = Approvals::new(
            |_| true,
            move |notice| seen.lock().expect("lock").push(notice),
        );
        let asking = Arc::clone(&approvals);
        let thread =
            std::thread::spawn(move || asking.ask_blocking("t:agent".into(), "Run QA".into()));
        let id = loop {
            let first = notices.lock().expect("lock").first().cloned();
            if let Some(Notice::Asked(question)) = first {
                assert_eq!(question.review, "Run QA");
                break question.id;
            }
            std::thread::yield_now();
        };
        let denied = Decision::Deny {
            message: "no".into(),
        };
        assert!(approvals.answer(id, denied.clone()));
        assert_eq!(thread.join().expect("join"), Some(denied));
        assert_eq!(
            notices.lock().expect("lock").last(),
            Some(&Notice::Closed(id))
        );
    }
}
