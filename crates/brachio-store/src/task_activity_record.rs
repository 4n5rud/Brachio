//! A bounded, per-task history of what happened to a task, kept across restarts.

use rusqlite::{params, Connection};

use crate::StoreError;

/// Entries kept per task; recording one more drops the oldest.
pub const MAX_TASK_ACTIVITY: usize = 500;

/// Longest detail kept, in characters. Details are titles and subjects, never prompts or output.
pub const MAX_ACTIVITY_DETAIL_CHARS: usize = 200;

/// What can be recorded. The interface chooses the wording for each.
pub const ACTIVITY_KINDS: [&str; 21] = [
    "permission_asked",
    "approval_answered",
    "approval_auto_allowed",
    "approval_closed_elsewhere",
    "question_asked",
    "auth_needed",
    "agent_error",
    "agent_finished",
    "agent_crashed",
    "status_changed",
    "child_created",
    "handoff_recorded",
    "handoff_approved",
    "handoff_rejected",
    "qa_run",
    "mail_received",
    "git_action",
    "prompt_sent",
    "agent_status",
    "restore_point",
    "pr_event",
];

/// One recorded event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEntry {
    /// Increases with every entry of the database, so it orders events even within a millisecond.
    pub id: i64,
    /// One of [`ACTIVITY_KINDS`].
    pub kind: String,
    /// A short, single-line description, such as a task title or a state name.
    pub detail: Option<String>,
    /// When it happened.
    pub created_at_ms: i64,
}

/// Records an event for a task and trims the task's history to [`MAX_TASK_ACTIVITY`].
///
/// # Errors
/// Fails with [`StoreError::Invalid`] for an unknown kind, and with a database error otherwise,
/// for instance for an unknown task.
pub fn record_activity(
    connection: &Connection,
    task_id: &str,
    kind: &str,
    detail: Option<&str>,
    at_ms: i64,
) -> Result<(), StoreError> {
    if !ACTIVITY_KINDS.contains(&kind) {
        return Err(StoreError::Invalid(format!(
            "unknown activity kind {kind:?}"
        )));
    }
    let detail = detail.map(one_line).filter(|text| !text.is_empty());
    connection.execute(
        "INSERT INTO task_activity (task_id, kind, detail, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
        params![task_id, kind, detail, at_ms],
    )?;
    connection.execute(
        "DELETE FROM task_activity WHERE task_id = ?1 AND id <= COALESCE(
            (SELECT id FROM task_activity WHERE task_id = ?1 ORDER BY id DESC LIMIT 1 OFFSET ?2), 0)",
        params![task_id, i64::try_from(MAX_TASK_ACTIVITY).unwrap_or(i64::MAX)],
    )?;
    Ok(())
}

/// Like [`record_activity`] for the task a message key like `<task id>:agent` addresses, and
/// quietly does nothing when no such task exists.
pub(crate) fn record_for_key(
    connection: &Connection,
    key: &str,
    kind: &str,
    detail: Option<&str>,
    at_ms: i64,
) -> Result<(), StoreError> {
    let task_id = key.split(':').next().unwrap_or(key);
    let known: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1)",
        [task_id],
        |row| row.get(0),
    )?;
    if known {
        record_activity(connection, task_id, kind, detail, at_ms)?;
    }
    Ok(())
}

/// A task's recorded events, newest first.
///
/// # Errors
/// Fails when the query fails.
pub fn task_activity(
    connection: &Connection,
    task_id: &str,
) -> Result<Vec<ActivityEntry>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT id, kind, detail, created_at_ms FROM task_activity
         WHERE task_id = ?1 ORDER BY id DESC",
    )?;
    let rows = statement.query_map([task_id], |row| {
        Ok(ActivityEntry {
            id: row.get(0)?,
            kind: row.get(1)?,
            detail: row.get(2)?,
            created_at_ms: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn one_line(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_ACTIVITY_DETAIL_CHARS)
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        decide_handoff, open_in_memory, save_qa_result, send_message, submit_handoff,
        HandoffReview, HandoffSource, NewHandoff, NewMessage, NewQaResult, TaskRecord,
    };

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

    fn kinds(connection: &Connection) -> Vec<String> {
        task_activity(connection, "t")
            .unwrap()
            .into_iter()
            .map(|entry| entry.kind)
            .collect()
    }

    #[test]
    fn entries_come_back_newest_first_even_within_one_millisecond() {
        let db = db();
        record_activity(&db, "t", "agent_finished", Some("first"), 5).unwrap();
        record_activity(&db, "t", "agent_crashed", Some("second"), 5).unwrap();
        let details: Vec<_> = task_activity(&db, "t")
            .unwrap()
            .into_iter()
            .map(|entry| entry.detail.unwrap())
            .collect();
        assert_eq!(details, ["second", "first"]);
    }

    #[test]
    fn history_is_bounded_per_task_and_keeps_the_newest() {
        let db = db();
        db.execute_batch(
            "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('other', 'r', 'O', 'p', 'b2', '/w2', 'new', 0);",
        )
        .unwrap();
        record_activity(&db, "other", "agent_finished", None, 0).unwrap();
        for n in 0..505 {
            record_activity(&db, "t", "status_changed", Some(&n.to_string()), n).unwrap();
        }
        let entries = task_activity(&db, "t").unwrap();
        assert_eq!(entries.len(), 500);
        assert_eq!(entries[0].detail.as_deref(), Some("504"));
        assert_eq!(entries[499].detail.as_deref(), Some("5"));
        assert_eq!(task_activity(&db, "other").unwrap().len(), 1);
    }

    #[test]
    fn unknown_kinds_are_refused_and_details_are_one_short_line() {
        let db = db();
        assert!(record_activity(&db, "t", "typed_a_secret", None, 0).is_err());
        let long = format!("line one\nline two\t{}", "x".repeat(500));
        record_activity(&db, "t", "git_action", Some(&long), 0).unwrap();
        let detail = task_activity(&db, "t").unwrap().remove(0).detail.unwrap();
        assert!(detail.starts_with("line one line two xxx"));
        assert_eq!(detail.chars().count(), 200);
    }

    #[test]
    fn deleting_a_task_deletes_its_history() {
        let db = db();
        record_activity(&db, "t", "agent_finished", None, 0).unwrap();
        db.execute("PRAGMA foreign_keys = ON", []).unwrap();
        TaskRecord::delete(&db, "t").unwrap();
        let left: i64 = db
            .query_row("SELECT COUNT(*) FROM task_activity", [], |row| row.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn handoff_review_and_qa_are_recorded_where_they_happen() {
        let mut db = db();
        db.execute_batch(
            "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('lead', 'r', 'L', 'p', 'lb', '/lw', 'new', 0);
             UPDATE tasks SET parent_task_id = 'lead' WHERE id = 't';",
        )
        .unwrap();
        submit_handoff(
            &mut db,
            &NewHandoff {
                id: "h",
                task_id: "t",
                commit_sha: SHA_A,
                base_sha: SHA_B,
                summary: "done",
                changed_paths: &[],
                reported_tests: &[],
                source: HandoffSource::ReportedByAgent,
                created_at_ms: 1,
            },
        )
        .unwrap();
        save_qa_result(
            &mut db,
            &NewQaResult {
                task_id: "t",
                command: "npm test",
                cwd: "/w",
                outcome: "passed",
                exit_code: Some(0),
                duration_ms: 1,
                commit_sha: SHA_A,
                verified_by_brachio: true,
                stdout: "",
                stderr: "",
                truncated: false,
                retry_allowed: false,
                created_at_ms: 2,
            },
        )
        .unwrap();
        decide_handoff(&mut db, "h", HandoffReview::Approved, None, 3).unwrap();
        assert_eq!(
            kinds(&db),
            [
                "status_changed",
                "handoff_approved",
                "qa_run",
                "handoff_recorded",
                "status_changed",
                "status_changed",
                "status_changed"
            ]
        );
        let qa = &task_activity(&db, "t").unwrap()[2];
        assert_eq!(qa.detail.as_deref(), Some("passed"));
    }

    #[test]
    fn a_message_is_recorded_once_for_its_receiver_even_when_delivered_twice() {
        let db = db();
        let message = NewMessage {
            id: "m1",
            from_key: "other:agent",
            to_key: "t:agent",
            kind: "status",
            priority: "normal",
            subject: "Tests are green",
            body: "never shown in the history",
            payload_json: None,
            reply_to: None,
            request_id: Some("req"),
            created_at_ms: 9,
        };
        send_message(&db, &message).unwrap();
        send_message(
            &db,
            &NewMessage {
                id: "m2",
                ..message
            },
        )
        .unwrap();
        let entries = task_activity(&db, "t").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, "mail_received");
        assert_eq!(entries[0].detail.as_deref(), Some("Tests are green"));
        let missing = NewMessage {
            id: "m3",
            to_key: "gone:agent",
            request_id: None,
            ..message
        };
        send_message(&db, &missing).unwrap();
    }
}
