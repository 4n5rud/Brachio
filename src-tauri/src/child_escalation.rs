//! Tells a lead that a child's agent stopped without handing off. It only notifies: the child's
//! state never changes here, and what to do about it stays the user's decision.

use brachio_core::task_graph::TaskState;
use brachio_store::{load_graph, send_message, Handoff, NewMessage, TaskRecord};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, State};

use crate::workspace::Database;

const MAX_DETAIL_CHARS: usize = 200;

/// What an agent's last status said about why it stopped.
fn stop_reason(state: &str, code: Option<i32>, detail: Option<&str>) -> Option<String> {
    let base = match (state, code) {
        ("exited", Some(code)) => format!("The agent exited with code {code}."),
        ("exited", None) => "The agent exited without an exit code.".to_owned(),
        ("idle", _) => "The agent finished its turn and is waiting for input.".to_owned(),
        _ => return None,
    };
    let detail: String = detail
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_DETAIL_CHARS)
        .collect();
    Some(if detail.is_empty() {
        base
    } else {
        format!("{base} Last status: {detail}")
    })
}

/// Mails the lead an `escalation` when a running child's agent stopped without a handoff.
/// Returns whether a new message was written; the same stop is reported once per handoff round.
///
/// # Errors
/// Fails when the database fails.
pub fn escalate_stop(
    connection: &Connection,
    task_id: &str,
    state: &str,
    code: Option<i32>,
    detail: Option<&str>,
    now_ms: i64,
    message_id: &str,
) -> Result<bool, String> {
    let Some(reason) = stop_reason(state, code, detail) else {
        return Ok(false);
    };
    let child = TaskRecord::get(connection, task_id).map_err(|e| e.to_string())?;
    let Some(lead_id) = child.parent_task_id.as_deref() else {
        return Ok(false);
    };
    let graph = load_graph(connection, lead_id).map_err(|e| e.to_string())?;
    if graph.state(task_id).ok() != Some(TaskState::Running) {
        return Ok(false);
    }
    let round = Handoff::latest(connection, task_id)
        .map_err(|e| e.to_string())?
        .map_or_else(|| "none".to_owned(), |handoff| handoff.id);
    let request_id = format!("stopped:{task_id}:{state}:{round}");
    let subject: String = format!("{} stopped without a handoff", child.title)
        .chars()
        .take(120)
        .collect();
    let (_, duplicate) = send_message(
        connection,
        &NewMessage {
            id: message_id,
            from_key: &format!("{task_id}:agent"),
            to_key: &format!("{lead_id}:agent"),
            kind: "escalation",
            priority: "high",
            subject: &subject,
            body: &reason,
            payload_json: None,
            reply_to: None,
            request_id: Some(&request_id),
            created_at_ms: now_ms,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(!duplicate)
}

/// The window saw a child's agent exit or go idle; tells the lead when it never handed off.
#[tauri::command]
pub fn child_agent_stopped(
    app: AppHandle,
    database: State<'_, Database>,
    task_id: String,
    state: String,
    code: Option<i32>,
    detail: Option<String>,
) -> Result<(), String> {
    let written = escalate_stop(
        &*database.lock()?,
        &task_id,
        &state,
        code,
        detail.as_deref(),
        crate::pr_status::now_ms(),
        &uuid::Uuid::new_v4().hyphenated().to_string(),
    )?;
    if written {
        if let Err(error) = app.emit(crate::mailbox::CHANGED_EVENT, ()) {
            tracing::debug!(%error, "could not tell the window about the escalation");
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use brachio_store::{open_in_memory, peek_inbox};

    use super::escalate_stop;

    fn database(child_state: Option<&str>) -> rusqlite::Connection {
        let connection = open_in_memory().unwrap();
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('lead', 'r', 'Lead', '', 'b0', '/w0', 'new', 0);
                 INSERT INTO tasks (id, repo_id, parent_task_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('kid', 'r', 'lead', 'Build API', '', 'b1', '/w1', 'new', 1);",
            )
            .unwrap();
        if let Some(state) = child_state {
            connection
                .execute(
                    "INSERT INTO task_orchestration (task_id, state, updated_at_ms) VALUES ('kid', ?1, 0)",
                    [state],
                )
                .unwrap();
        }
        connection
    }

    fn inbox(connection: &rusqlite::Connection) -> Vec<brachio_store::Message> {
        peek_inbox(connection, "lead:agent", 10).unwrap()
    }

    #[test]
    fn a_running_child_that_exits_without_a_handoff_escalates_once_and_stays_running() {
        let connection = database(Some("running"));
        assert!(escalate_stop(
            &connection,
            "kid",
            "exited",
            Some(1),
            Some("crashed"),
            5,
            "m1"
        )
        .unwrap());
        assert!(!escalate_stop(&connection, "kid", "exited", Some(1), None, 6, "m2").unwrap());
        let mail = inbox(&connection);
        assert_eq!(mail.len(), 1);
        assert_eq!(mail[0].kind, "escalation");
        assert_eq!(mail[0].to_key, "lead:agent");
        assert_eq!(mail[0].subject, "Build API stopped without a handoff");
        assert_eq!(
            mail[0].body,
            "The agent exited with code 1. Last status: crashed"
        );
        let state: String = connection
            .query_row(
                "SELECT state FROM task_orchestration WHERE task_id = 'kid'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "running");
    }

    #[test]
    fn a_child_that_is_not_running_or_a_working_agent_does_not_escalate() {
        let connection = database(Some("review_required"));
        assert!(!escalate_stop(&connection, "kid", "idle", None, None, 5, "m1").unwrap());
        let connection = database(Some("running"));
        assert!(!escalate_stop(&connection, "kid", "working", None, None, 5, "m1").unwrap());
        assert!(inbox(&connection).is_empty());
    }
}
