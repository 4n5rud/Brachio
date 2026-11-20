//! Messages between agent sessions: what the app does with a request from `brachio send`,
//! `check`, `inbox` or `peers`, and what the interface reads to show unread mail.

use brachio_runtime::mailbox::{
    MailMessage, MailRequest, MailResponse, Peer, SendMail, DEFAULT_BATCH, MAX_BATCH,
};
use brachio_store::{
    peek_unread, recent_messages, send_message, take_unread, unread_counts, Message, NewMessage,
    TaskRecord,
};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::terminal_authorization;
use crate::workspace::Database;

/// Tells the interface that unread counts changed.
pub const CHANGED_EVENT: &str = "mailbox://changed";
const PREVIEW_CHARS: usize = 200;
const PRIORITIES: [&str; 3] = ["low", "normal", "high"];

/// Answers one request. The second value says whether unread counts changed.
pub fn handle(
    connection: &mut Connection,
    request: MailRequest,
    now_ms: i64,
    new_id: &str,
) -> (MailResponse, bool) {
    let outcome = match request {
        MailRequest::Send(mail) => send(connection, &mail, now_ms, new_id),
        MailRequest::Check { key, limit, peek } => check(connection, &key, limit, peek, now_ms),
        MailRequest::Inbox { key, limit } => inbox(connection, &key, limit),
        MailRequest::Peers { key } => peers(connection, &key),
    };
    outcome.unwrap_or_else(|message| (MailResponse::Error { message }, false))
}

type Outcome = Result<(MailResponse, bool), String>;

fn agent_owner(connection: &Connection, key: &str) -> Result<TaskRecord, String> {
    let owner = terminal_authorization::resolve(connection, key)
        .map_err(|_| "This terminal does not belong to a Brachio task.".to_owned())?;
    if owner.agent.is_none() {
        return Err("Only an agent terminal can use messages, not a plain shell.".to_owned());
    }
    Ok(owner.task)
}

fn send(connection: &Connection, mail: &SendMail, now_ms: i64, new_id: &str) -> Outcome {
    agent_owner(connection, &mail.from)?;
    let to = resolve_target(connection, &mail.to)?;
    if to == mail.from {
        return Err("A message cannot be sent to the terminal that sends it.".to_owned());
    }
    let priority = mail.priority.as_deref().unwrap_or("normal");
    if !PRIORITIES.contains(&priority) {
        return Err(format!(
            "Priority must be one of {}.",
            PRIORITIES.join(", ")
        ));
    }
    let (stored, duplicate) = send_message(
        connection,
        &NewMessage {
            id: new_id,
            from_key: &mail.from,
            to_key: &to,
            kind: &mail.kind,
            priority,
            subject: &mail.subject,
            body: &mail.body,
            payload_json: mail.payload.as_deref(),
            reply_to: mail.reply_to.as_deref(),
            request_id: mail.request_id.as_deref(),
            created_at_ms: now_ms,
        },
    )
    .map_err(|error| error.to_string())?;
    Ok((
        MailResponse::Sent {
            id: stored.id,
            thread_id: stored.thread_id,
            to,
            duplicate,
        },
        !duplicate,
    ))
}

/// A terminal key, a task id or its start, or a task title.
fn resolve_target(connection: &Connection, text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.contains(':') {
        return agent_owner(connection, text).map(|_| text.to_owned());
    }
    let tasks = TaskRecord::list_open(connection).map_err(|error| error.to_string())?;
    let matching = |predicate: &dyn Fn(&TaskRecord) -> bool| -> Vec<&TaskRecord> {
        tasks.iter().filter(|task| predicate(task)).collect()
    };
    let lowered = text.to_lowercase();
    let mut found = matching(&|task| task.id == text);
    if found.is_empty() {
        found = matching(&|task| task.title.to_lowercase() == lowered);
    }
    if found.is_empty() && text.len() >= 4 {
        found = matching(&|task| task.id.starts_with(text));
    }
    match found.as_slice() {
        [task] => Ok(format!("{}:agent", task.id)),
        [] => Err(format!(
            "No open task matches {text:?}. Run `brachio peers` to see where a message can go."
        )),
        _ => Err(format!(
            "{text:?} matches more than one task. Use the key from `brachio peers`."
        )),
    }
}

fn check(
    connection: &mut Connection,
    key: &str,
    limit: Option<usize>,
    peek: bool,
    now_ms: i64,
) -> Outcome {
    agent_owner(connection, key)?;
    let limit = limit.unwrap_or(DEFAULT_BATCH).clamp(1, MAX_BATCH);
    let batch = if peek {
        peek_unread(connection, key, limit)
    } else {
        take_unread(connection, key, limit, now_ms)
    }
    .map_err(|error| error.to_string())?;
    let changed = !peek && !batch.is_empty();
    let messages = batch
        .into_iter()
        .map(|message| mail_message(connection, message, false))
        .collect();
    Ok((
        MailResponse::Messages {
            messages,
            unread_left: unread_for(connection, key)?,
        },
        changed,
    ))
}

fn inbox(connection: &Connection, key: &str, limit: Option<usize>) -> Outcome {
    agent_owner(connection, key)?;
    let limit = limit.unwrap_or(DEFAULT_BATCH).clamp(1, MAX_BATCH);
    let recent = recent_messages(connection, key, limit).map_err(|error| error.to_string())?;
    let messages = recent
        .into_iter()
        .map(|message| mail_message(connection, message, true))
        .collect();
    Ok((
        MailResponse::Messages {
            messages,
            unread_left: unread_for(connection, key)?,
        },
        false,
    ))
}

fn peers(connection: &Connection, key: &str) -> Outcome {
    agent_owner(connection, key)?;
    let tasks = TaskRecord::list_open(connection).map_err(|error| error.to_string())?;
    let mut peers = Vec::new();
    for task in tasks {
        let second_tab: Option<String> = connection
            .query_row(
                "SELECT tab_id FROM task_secondary_tabs WHERE task_id = ?1 AND tab_id IS NOT NULL",
                [&task.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some(tab_id) = second_tab {
            peers.push(Peer {
                key: format!("{}:agent-{tab_id}", task.id),
                title: format!("{} (second agent)", task.title),
                branch: task.branch.clone(),
            });
        }
        peers.push(Peer {
            key: format!("{}:agent", task.id),
            title: task.title,
            branch: task.branch,
        });
    }
    peers.retain(|peer| peer.key != key);
    Ok((MailResponse::Peers { peers }, false))
}

fn unread_for(connection: &Connection, key: &str) -> Result<u32, String> {
    let counts = unread_counts(connection).map_err(|error| error.to_string())?;
    Ok(counts
        .into_iter()
        .find_map(|(to, count)| (to == key).then_some(count))
        .unwrap_or(0))
}

fn mail_message(connection: &Connection, message: Message, report_read: bool) -> MailMessage {
    let from_title = message
        .from_key
        .split_once(':')
        .and_then(|(task_id, _)| TaskRecord::get(connection, task_id).ok())
        .map(|task| task.title);
    MailMessage {
        id: message.id,
        thread_id: message.thread_id,
        from: message.from_key,
        to: message.to_key,
        from_title,
        kind: message.kind,
        priority: message.priority,
        subject: message.subject,
        body: message.body,
        payload: message.payload_json,
        created_at_ms: message.created_at_ms,
        read: report_read && message.read_at_ms.is_some(),
    }
}

/// Unread messages waiting for one terminal.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnreadRow {
    tab_key: String,
    count: u32,
}

/// A waiting message as the interface lists it.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InboxRow {
    id: String,
    from_key: String,
    subject: String,
    preview: String,
    created_at_ms: i64,
}

/// How many unread messages each terminal has.
#[tauri::command]
pub fn mailbox_unread(database: State<'_, Database>) -> Result<Vec<UnreadRow>, String> {
    let connection = database.lock()?;
    let counts = unread_counts(&connection).map_err(|error| error.to_string())?;
    Ok(counts
        .into_iter()
        .map(|(tab_key, count)| UnreadRow { tab_key, count })
        .collect())
}

/// The unread messages of one terminal, shortened, without marking them read.
#[tauri::command]
pub fn mailbox_inbox(
    database: State<'_, Database>,
    tab_key: String,
) -> Result<Vec<InboxRow>, String> {
    let connection = database.lock()?;
    let waiting =
        peek_unread(&connection, &tab_key, MAX_BATCH).map_err(|error| error.to_string())?;
    Ok(waiting
        .into_iter()
        .map(|message| InboxRow {
            preview: message.body.chars().take(PREVIEW_CHARS).collect(),
            id: message.id,
            from_key: message.from_key,
            subject: message.subject,
            created_at_ms: message.created_at_ms,
        })
        .collect())
}

/// Runs a request from the socket against the app's database and tells the interface when the
/// unread counts changed.
pub fn serve(app: &AppHandle, request: MailRequest) -> MailResponse {
    let database = app.state::<Database>();
    let Ok(mut connection) = database.lock() else {
        return MailResponse::Error {
            message: "Brachio's database is busy; try again.".to_owned(),
        };
    };
    let (response, changed) = handle(
        &mut connection,
        request,
        crate::pr_status::now_ms(),
        &uuid::Uuid::new_v4().hyphenated().to_string(),
    );
    drop(connection);
    if changed {
        if let Err(error) = app.emit(CHANGED_EVENT, ()) {
            tracing::debug!(%error, "could not tell the window about new mail");
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use brachio_runtime::mailbox::{MailRequest, MailResponse, SendMail};
    use brachio_store::open_in_memory;

    use super::handle;

    fn database() -> rusqlite::Connection {
        let connection = open_in_memory().expect("open database");
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('aaaa1111', 'r', 'Build API', '', 'b1', '/w1', 'new', 0),
                        ('bbbb2222', 'r', 'Write docs', '', 'b2', '/w2', 'new', 0);",
            )
            .expect("insert tasks");
        connection
    }

    fn mail(from: &str, to: &str, subject: &str) -> MailRequest {
        MailRequest::Send(SendMail {
            from: from.into(),
            to: to.into(),
            kind: "question".into(),
            priority: None,
            subject: subject.into(),
            body: "details".into(),
            payload: None,
            reply_to: None,
            request_id: None,
        })
    }

    #[test]
    fn a_message_reaches_the_task_named_by_title_and_is_read_once() {
        let mut connection = database();
        let (answer, changed) = handle(
            &mut connection,
            mail("aaaa1111:agent", "write docs", "Which port?"),
            10,
            "m1",
        );
        assert!(changed);
        assert!(matches!(answer, MailResponse::Sent { ref to, .. } if to == "bbbb2222:agent"));

        let check = |connection: &mut rusqlite::Connection| {
            handle(
                connection,
                MailRequest::Check {
                    key: "bbbb2222:agent".into(),
                    limit: None,
                    peek: false,
                },
                20,
                "x",
            )
        };
        let (first, changed) = check(&mut connection);
        assert!(changed);
        let MailResponse::Messages {
            messages,
            unread_left,
        } = first
        else {
            panic!("expected messages");
        };
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].subject, "Which port?");
        assert_eq!(messages[0].from_title.as_deref(), Some("Build API"));
        assert_eq!(unread_left, 0);
        let (second, changed) = check(&mut connection);
        assert!(!changed);
        assert!(
            matches!(second, MailResponse::Messages { ref messages, .. } if messages.is_empty())
        );
    }

    #[test]
    fn only_agents_of_open_tasks_can_send_and_nobody_can_message_themselves() {
        let mut connection = database();
        for (from, to) in [
            ("ghost:agent", "bbbb2222"),
            ("aaaa1111:shell", "bbbb2222"),
            ("aaaa1111:agent", "aaaa1111"),
            ("aaaa1111:agent", "nobody"),
        ] {
            let (answer, changed) = handle(&mut connection, mail(from, to, "hi"), 1, "m");
            assert!(
                matches!(answer, MailResponse::Error { .. }),
                "{from} -> {to}"
            );
            assert!(!changed);
        }
    }

    #[test]
    fn peers_list_the_other_open_tasks_with_their_keys() {
        let mut connection = database();
        let (answer, _) = handle(
            &mut connection,
            MailRequest::Peers {
                key: "aaaa1111:agent".into(),
            },
            1,
            "x",
        );
        let MailResponse::Peers { peers } = answer else {
            panic!("expected peers")
        };
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].key, "bbbb2222:agent");
        assert_eq!(peers[0].title, "Write docs");
    }

    #[test]
    fn a_second_agent_tab_is_listed_and_can_receive_mail() {
        let mut connection = database();
        let tab = "123e4567-e89b-12d3-a456-426614174000";
        connection
            .execute(
                "INSERT INTO task_secondary_tabs (task_id, tab_id) VALUES ('bbbb2222', ?1)",
                [tab],
            )
            .expect("register tab");
        let key = format!("bbbb2222:agent-{tab}");
        let (answer, _) = handle(
            &mut connection,
            MailRequest::Peers {
                key: "aaaa1111:agent".into(),
            },
            1,
            "x",
        );
        let MailResponse::Peers { peers } = answer else {
            panic!("expected peers")
        };
        let second = peers.iter().find(|peer| peer.key == key).expect("listed");
        assert_eq!(second.title, "Write docs (second agent)");
        assert_eq!(second.branch, "b2");

        let (answer, changed) =
            handle(&mut connection, mail("aaaa1111:agent", &key, "hi"), 2, "m1");
        assert!(changed);
        assert!(matches!(answer, MailResponse::Sent { ref to, .. } if *to == key));
    }
}
