//! Decision gates: a child asks the user to pick one of its options. Open gates hold back the
//! tasks that depend on the child, and only the user's answer, sent to the child as a mailbox
//! reply, closes one.

use brachio_store::{
    gate_options, pending_gate, send_message, NewMessage, PendingGate, TaskRecord,
};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, State};

use crate::mailbox::agent_owner;
use crate::workspace::Database;

/// Checks a `decision_gate` before it is stored: only a child may raise one, to its own lead,
/// with options to choose from.
///
/// # Errors
/// A message for the sender.
pub fn check_new_gate(
    connection: &Connection,
    from: &str,
    to: &str,
    payload: Option<&str>,
) -> Result<(), String> {
    let child = agent_owner(connection, from)?;
    let lead = child
        .parent_task_id
        .ok_or_else(|| "Only a child task can raise a decision gate.".to_owned())?;
    if to != format!("{lead}:agent") {
        return Err("A decision gate goes to the lead task.".to_owned());
    }
    gate_options(payload).map(|_| ())
}

/// Records the user's choice as a reply to the gate, which closes it.
///
/// # Errors
/// Fails when the gate is gone or answered, the choice is not one of its options, or the
/// reply cannot be stored.
pub fn answer_gate(
    connection: &Connection,
    gate_id: &str,
    choice: usize,
    now_ms: i64,
    reply_id: &str,
) -> Result<PendingGate, String> {
    let gate = pending_gate(connection, gate_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "This decision was already answered or no longer exists.".to_owned())?;
    let option = gate
        .options
        .get(choice)
        .ok_or_else(|| "That is not one of the options.".to_owned())?;
    let child = TaskRecord::get(connection, &gate.task_id).map_err(|e| e.to_string())?;
    let lead_id = child
        .parent_task_id
        .ok_or_else(|| "Only a child task has a decision gate.".to_owned())?;
    send_message(
        connection,
        &NewMessage {
            id: reply_id,
            from_key: &format!("{lead_id}:agent"),
            to_key: &format!("{}:agent", gate.task_id),
            kind: "status",
            priority: "high",
            subject: &format!("Decision: {option}"),
            body: &format!("The user chose \"{option}\" for: {}", gate.subject),
            payload_json: None,
            reply_to: Some(gate_id),
            request_id: None,
            created_at_ms: now_ms,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(gate)
}

/// The user chose one of a gate's options.
#[tauri::command]
pub fn decision_gate_answer(
    app: AppHandle,
    database: State<'_, Database>,
    gate_id: String,
    choice: usize,
) -> Result<(), String> {
    answer_gate(
        &*database.lock()?,
        &gate_id,
        choice,
        crate::pr_status::now_ms(),
        &uuid::Uuid::new_v4().hyphenated().to_string(),
    )?;
    crate::orchestration::schedule_all(&app);
    for event in [
        crate::mailbox::CHANGED_EVENT,
        crate::orchestration::CHANGED_EVENT,
    ] {
        if let Err(error) = app.emit(event, ()) {
            tracing::debug!(%error, "could not tell the window about the decision");
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use brachio_runtime::mailbox::{MailRequest, MailResponse, SendMail};
    use brachio_store::{open_in_memory, pending_gates};

    use super::answer_gate;
    use crate::mailbox::handle;

    fn database() -> rusqlite::Connection {
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
        connection
    }

    fn gate(from: &str, to: &str, payload: Option<&str>) -> MailRequest {
        MailRequest::Send(SendMail {
            from: from.into(),
            to: to.into(),
            kind: "decision_gate".into(),
            priority: None,
            subject: "Which database?".into(),
            body: String::new(),
            payload: payload.map(str::to_owned),
            reply_to: None,
            request_id: None,
        })
    }

    #[test]
    fn a_child_raises_a_gate_and_the_users_choice_reaches_it_as_a_reply() {
        let mut connection = database();
        let options = Some(r#"{"options":["SQLite","Postgres"]}"#);
        let (sent, _) = handle(
            &mut connection,
            gate("kid:agent", "parent", options),
            1,
            "g1",
        );
        assert!(matches!(sent, MailResponse::Sent { .. }));
        assert_eq!(pending_gates(&connection, "kid").unwrap().len(), 1);

        assert!(answer_gate(&connection, "g1", 5, 2, "r1").is_err());
        answer_gate(&connection, "g1", 1, 2, "r1").unwrap();
        assert!(pending_gates(&connection, "kid").unwrap().is_empty());
        assert!(answer_gate(&connection, "g1", 0, 3, "r2").is_err());

        let (reply, _) = handle(
            &mut connection,
            MailRequest::Check {
                key: "kid:agent".into(),
                limit: None,
                peek: true,
            },
            4,
            "c",
        );
        let MailResponse::Messages { messages, .. } = reply else {
            panic!("expected messages");
        };
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].subject, "Decision: Postgres");
    }

    #[test]
    fn gates_need_a_child_a_lead_recipient_and_options() {
        let mut connection = database();
        let good = Some(r#"{"options":["A","B"]}"#);
        for request in [
            gate("lead:agent", "kid", good),
            gate("kid:agent", "parent", None),
            gate("kid:agent", "parent", Some(r#"{"options":["only"]}"#)),
        ] {
            let (answer, changed) = handle(&mut connection, request, 1, "m");
            assert!(matches!(answer, MailResponse::Error { .. }));
            assert!(!changed);
        }
    }
}
