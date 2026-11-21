//! QA runs Brachio executed for a task, and their link to a handoff of the same commit.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::StoreError;

/// How many QA runs a task keeps. Storing another run deletes the oldest beyond this.
pub const MAX_QA_RESULTS_PER_TASK: i64 = 20;

/// A finished QA run to store.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct NewQaResult<'a> {
    /// The task it ran for.
    pub task_id: &'a str,
    /// The command line that ran.
    pub command: &'a str,
    /// The directory it ran in.
    pub cwd: &'a str,
    /// `passed`, `failed`, `timedOut`, `outputLimit` or `processError`.
    pub outcome: &'a str,
    /// Exit code, when the process had one.
    pub exit_code: Option<i32>,
    /// Run time in milliseconds.
    pub duration_ms: i64,
    /// The commit checked out for the run.
    pub commit_sha: &'a str,
    /// Brachio ran it and HEAD stayed the same.
    pub verified_by_brachio: bool,
    /// Captured output, already capped by the runner.
    pub stdout: &'a str,
    /// Captured error output, already capped by the runner.
    pub stderr: &'a str,
    /// Some output was dropped.
    pub truncated: bool,
    /// Whether the feedback gate would allow another automatic attempt.
    pub retry_allowed: bool,
    /// Creation time.
    pub created_at_ms: i64,
}

/// A stored QA run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct QaResultRecord {
    /// Row id.
    pub id: i64,
    /// The command line that ran.
    pub command: String,
    /// The directory it ran in.
    pub cwd: String,
    /// `passed`, `failed`, `timedOut`, `outputLimit` or `processError`.
    pub outcome: String,
    /// Exit code, when the process had one.
    pub exit_code: Option<i32>,
    /// Run time in milliseconds.
    pub duration_ms: i64,
    /// The commit checked out for the run.
    pub commit_sha: String,
    /// Brachio ran it and HEAD stayed the same.
    pub verified_by_brachio: bool,
    /// Captured output.
    pub stdout: String,
    /// Captured error output.
    pub stderr: String,
    /// Some output was dropped.
    pub truncated: bool,
    /// Whether the feedback gate allowed another automatic attempt.
    pub retry_allowed: bool,
    /// Creation time.
    pub created_at_ms: i64,
    /// A handoff of the same task and commit has this run attached.
    pub matches_handoff: bool,
}

/// Stores a run and, when Brachio verified it, attaches it to the task's handoffs of the same
/// commit. A handoff of any other commit and the agent's `reported_tests` are left alone.
///
/// # Errors
/// Fails when the database does, for instance for an unknown task.
pub fn save_qa_result(
    connection: &mut Connection,
    result: &NewQaResult<'_>,
) -> Result<i64, StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO qa_results (task_id, command, cwd, outcome, exit_code, duration_ms,
                                 commit_sha, verified_by_brachio, stdout, stderr, truncated,
                                 retry_allowed, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            result.task_id,
            result.command,
            result.cwd,
            result.outcome,
            result.exit_code,
            result.duration_ms,
            result.commit_sha,
            result.verified_by_brachio,
            result.stdout,
            result.stderr,
            result.truncated,
            result.retry_allowed,
            result.created_at_ms,
        ],
    )?;
    let id = transaction.last_insert_rowid();
    transaction.execute(
        "DELETE FROM qa_results WHERE task_id = ?1
         AND id NOT IN (SELECT id FROM qa_results WHERE task_id = ?1 ORDER BY id DESC LIMIT ?2)",
        params![result.task_id, MAX_QA_RESULTS_PER_TASK],
    )?;
    if result.verified_by_brachio {
        transaction.execute(
            "UPDATE task_handoffs SET verified_qa_result_id = ?1
             WHERE task_id = ?2 AND commit_sha = ?3",
            params![id, result.task_id, result.commit_sha],
        )?;
    }
    transaction.commit()?;
    Ok(id)
}

/// Deletes every stored QA run of a task, including its captured output. Returns how many.
///
/// # Errors
/// Fails when the database does.
pub fn delete_qa_results(connection: &Connection, task_id: &str) -> Result<usize, StoreError> {
    Ok(connection.execute("DELETE FROM qa_results WHERE task_id = ?1", [task_id])?)
}

/// The newest QA run of a task, if any.
///
/// # Errors
/// Fails when the query fails.
pub fn latest_qa_result(
    connection: &Connection,
    task_id: &str,
) -> Result<Option<QaResultRecord>, StoreError> {
    select_qa_result(
        connection,
        "q.task_id = ?1 ORDER BY q.id DESC LIMIT 1",
        task_id,
    )
}

/// One QA run by row id, if it exists.
///
/// # Errors
/// Fails when the query fails.
pub fn qa_result_by_id(
    connection: &Connection,
    id: i64,
) -> Result<Option<QaResultRecord>, StoreError> {
    select_qa_result(connection, "q.id = ?1", id)
}

fn select_qa_result(
    connection: &Connection,
    condition: &str,
    value: impl rusqlite::ToSql,
) -> Result<Option<QaResultRecord>, StoreError> {
    Ok(connection
        .query_row(
            &format!("SELECT q.id, q.command, q.cwd, q.outcome, q.exit_code, q.duration_ms, q.commit_sha,
                    q.verified_by_brachio, q.stdout, q.stderr, q.truncated, q.retry_allowed,
                    q.created_at_ms,
                    EXISTS(SELECT 1 FROM task_handoffs h WHERE h.verified_qa_result_id = q.id)
             FROM qa_results q WHERE {condition}"),
            [value],
            |row| {
                Ok(QaResultRecord {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    cwd: row.get(2)?,
                    outcome: row.get(3)?,
                    exit_code: row.get(4)?,
                    duration_ms: row.get(5)?,
                    commit_sha: row.get(6)?,
                    verified_by_brachio: row.get(7)?,
                    stdout: row.get(8)?,
                    stderr: row.get(9)?,
                    truncated: row.get(10)?,
                    retry_allowed: row.get(11)?,
                    created_at_ms: row.get(12)?,
                    matches_handoff: row.get(13)?,
                })
            },
        )
        .optional()?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{open_in_memory, record_handoff, Handoff, HandoffSource, NewHandoff};

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn db() -> Connection {
        let connection = open_in_memory().unwrap();
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);",
            )
            .unwrap();
        connection
    }

    fn run<'a>(sha: &'a str, stdout: &'a str) -> NewQaResult<'a> {
        NewQaResult {
            task_id: "t",
            command: "npm test",
            cwd: "/w",
            outcome: "failed",
            exit_code: Some(1),
            duration_ms: 1500,
            commit_sha: sha,
            verified_by_brachio: true,
            stdout,
            stderr: "boom",
            truncated: false,
            retry_allowed: false,
            created_at_ms: 7,
        }
    }

    fn handoff(connection: &Connection, id: &str, sha: &str) {
        record_handoff(
            connection,
            &NewHandoff {
                id,
                task_id: "t",
                commit_sha: sha,
                base_sha: SHA_B,
                summary: "done",
                changed_paths: &[],
                reported_tests: &["agent says ok".to_owned()],
                source: HandoffSource::ReportedByAgent,
                created_at_ms: 1,
            },
        )
        .unwrap();
    }

    #[test]
    fn the_latest_result_is_saved_and_loaded_whole() {
        let mut db = db();
        assert_eq!(latest_qa_result(&db, "t").unwrap(), None);
        save_qa_result(&mut db, &run(SHA_A, "first")).unwrap();
        save_qa_result(&mut db, &run(SHA_A, "second")).unwrap();
        let latest = latest_qa_result(&db, "t").unwrap().unwrap();
        assert_eq!(latest.stdout, "second");
        assert_eq!(latest.stderr, "boom");
        assert_eq!(latest.command, "npm test");
        assert_eq!(latest.cwd, "/w");
        assert_eq!(latest.outcome, "failed");
        assert_eq!(latest.exit_code, Some(1));
        assert_eq!(latest.duration_ms, 1500);
        assert_eq!(latest.commit_sha, SHA_A);
        assert!(latest.verified_by_brachio && !latest.retry_allowed && !latest.matches_handoff);
        assert_eq!(latest.created_at_ms, 7);
    }

    #[test]
    fn deleting_the_task_deletes_its_results() {
        let mut db = db();
        save_qa_result(&mut db, &run(SHA_A, "x")).unwrap();
        db.execute("DELETE FROM tasks WHERE id = 't'", []).unwrap();
        assert_eq!(latest_qa_result(&db, "t").unwrap(), None);
    }

    #[test]
    fn a_verified_run_attaches_to_the_handoff_of_the_same_commit_only() {
        let mut db = db();
        handoff(&db, "h-a", SHA_A);
        handoff(&db, "h-b", SHA_B);
        let id = save_qa_result(&mut db, &run(SHA_A, "x")).unwrap();
        let handoffs = Handoff::for_task(&db, "t").unwrap();
        assert_eq!(handoffs[0].verified_qa_result_id, Some(id));
        assert_eq!(handoffs[0].reported_tests, vec!["agent says ok"]);
        assert_eq!(handoffs[1].verified_qa_result_id, None);
        assert!(latest_qa_result(&db, "t").unwrap().unwrap().matches_handoff);
    }

    #[test]
    fn a_run_for_another_commit_or_unverified_attaches_nowhere() {
        let mut db = db();
        handoff(&db, "h-a", SHA_A);
        save_qa_result(&mut db, &run(SHA_B, "x")).unwrap();
        let mut unverified = run(SHA_A, "y");
        unverified.verified_by_brachio = false;
        save_qa_result(&mut db, &unverified).unwrap();
        let handoffs = Handoff::for_task(&db, "t").unwrap();
        assert_eq!(handoffs[0].verified_qa_result_id, None);
        assert!(!latest_qa_result(&db, "t").unwrap().unwrap().matches_handoff);
    }

    fn stored_count(db: &Connection) -> i64 {
        db.query_row("SELECT COUNT(*) FROM qa_results", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn only_the_newest_results_of_a_task_are_kept() {
        let mut db = db();
        for index in 0..MAX_QA_RESULTS_PER_TASK + 3 {
            save_qa_result(&mut db, &run(SHA_A, &format!("run {index}"))).unwrap();
        }
        assert_eq!(stored_count(&db), 20);
        let latest = latest_qa_result(&db, "t").unwrap().unwrap();
        assert_eq!(latest.stdout, "run 22");
        let oldest: String = db
            .query_row(
                "SELECT stdout FROM qa_results ORDER BY id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(oldest, "run 3");
    }

    #[test]
    fn deleting_a_tasks_output_leaves_other_tasks_alone() {
        let mut db = db();
        db.execute_batch(
            "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('u', 'r', 'U', 'p', 'b2', '/w2', 'new', 0);",
        )
        .unwrap();
        save_qa_result(&mut db, &run(SHA_A, "mine")).unwrap();
        let mut other = run(SHA_A, "theirs");
        other.task_id = "u";
        save_qa_result(&mut db, &other).unwrap();
        assert_eq!(delete_qa_results(&db, "t").unwrap(), 1);
        assert_eq!(latest_qa_result(&db, "t").unwrap(), None);
        assert!(latest_qa_result(&db, "u").unwrap().is_some());
    }

    #[test]
    fn deleting_a_task_deletes_its_stored_output() {
        let mut db = db();
        save_qa_result(&mut db, &run(SHA_A, "secret-ish")).unwrap();
        crate::TaskRecord::delete(&db, "t").unwrap();
        assert_eq!(stored_count(&db), 0);
    }
}
