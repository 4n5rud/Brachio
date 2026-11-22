//! Messages between agent sessions: what the app does with a request from `brachio send`,
//! `check`, `inbox` or `peers`, and what the interface reads to show unread mail.

use std::time::Duration;

use brachio_runtime::mailbox::{
    ChildTaskInfo, MailMessage, MailRequest, MailResponse, Peer, ReportedChild, SendMail,
    DEFAULT_BATCH, MAX_BATCH, MAX_WAIT_SECS,
};
use brachio_store::{
    child_reports, dismiss_messages, mark_read, peek_inbox, peek_next_batch, pending_gate,
    recent_messages, reply_recipient, send_message, unread_counts, Message, NewMessage, TaskRecord,
};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::terminal_authorization;
use crate::workspace::Database;

/// Tells the interface that unread counts changed.
pub const CHANGED_EVENT: &str = "mailbox://changed";
const PREVIEW_CHARS: usize = 200;
const PRIORITIES: [&str; 3] = ["low", "normal", "high"];
const REPORT_SUBJECT_CHARS: usize = 120;
/// How often a waiting lead's children are looked at again.
pub const WAIT_INTERVAL: Duration = Duration::from_secs(1);

/// Answers one request. The second value says whether unread counts changed.
pub fn handle(
    connection: &mut Connection,
    request: MailRequest,
    now_ms: i64,
    new_id: &str,
) -> (MailResponse, bool) {
    let outcome = match request {
        MailRequest::Send(mail) => send(connection, &mail, now_ms, new_id),
        MailRequest::Check { key, limit, .. } => check(connection, &key, limit),
        MailRequest::Ack { .. } => Err("An acknowledgement answers a check.".to_owned()),
        MailRequest::Inbox { key, limit } => inbox(connection, &key, limit),
        MailRequest::Peers { key } => peers(connection, &key),
        MailRequest::CreateTask { .. } => Err("Creating tasks needs the user's answer.".to_owned()),
        MailRequest::Handoff {
            key,
            summary,
            tests,
        } => crate::orchestration::handoff_request(
            connection, &key, &summary, &tests, now_ms, new_id,
        )
        .map(|response| (response, false)),
        MailRequest::ListTasks { key } => list_tasks(connection, &key),
        MailRequest::WaitTasks { key, .. } => child_status(connection, &key),
    };
    outcome.unwrap_or_else(|message| (MailResponse::Error { message }, false))
}

type Outcome = Result<(MailResponse, bool), String>;

pub(crate) fn agent_owner(connection: &Connection, key: &str) -> Result<TaskRecord, String> {
    let owner = terminal_authorization::resolve(connection, key)
        .map_err(|_| "This terminal does not belong to a Brachio task.".to_owned())?;
    if owner.agent.is_none() {
        return Err("Only an agent terminal can use messages, not a plain shell.".to_owned());
    }
    Ok(owner.task)
}

fn send(connection: &Connection, mail: &SendMail, now_ms: i64, new_id: &str) -> Outcome {
    let sender = agent_owner(connection, &mail.from)?;
    if let Some(parent) = mail.reply_to.as_deref() {
        if pending_gate(connection, parent)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("Only the user can answer a decision gate.".to_owned());
        }
    }
    let to = match mail.reply_to.as_deref() {
        Some(parent) => {
            let recipient = reply_recipient(connection, parent, &mail.from)
                .map_err(|error| error.to_string())?;
            agent_owner(connection, &recipient)?;
            if !mail.to.trim().is_empty()
                && resolve_target(connection, &sender, &mail.to)? != recipient
            {
                return Err("A reply must go to the other participant in the thread.".to_owned());
            }
            recipient
        }
        None => resolve_target(connection, &sender, &mail.to)?,
    };
    if to == mail.from {
        return Err("A message cannot be sent to the terminal that sends it.".to_owned());
    }
    if mail.kind == "decision_gate" {
        crate::decision_gate::check_new_gate(connection, &mail.from, &to, mail.payload.as_deref())?;
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
            to: stored.to_key,
            duplicate,
        },
        !duplicate,
    ))
}

/// `parent`, a terminal key, a task id or its start, or a task title.
fn resolve_target(
    connection: &Connection,
    sender: &TaskRecord,
    text: &str,
) -> Result<String, String> {
    let text = text.trim();
    if text == "parent" {
        let parent = sender
            .parent_task_id
            .as_deref()
            .ok_or_else(|| "this task has no parent".to_owned())?;
        let key = format!("{parent}:agent");
        return agent_owner(connection, &key).map(|_| key);
    }
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

/// The next unread messages, still unread: the socket marks them with [`mark_delivered`] once
/// the client acknowledged them, so a client that left cannot lose them.
fn check(connection: &mut Connection, key: &str, limit: Option<usize>) -> Outcome {
    agent_owner(connection, key)?;
    let limit = limit.unwrap_or(DEFAULT_BATCH).clamp(1, MAX_BATCH);
    let batch = peek_next_batch(connection, key, limit).map_err(|error| error.to_string())?;
    let messages = batch
        .into_iter()
        .map(|message| mail_message(connection, message, false))
        .collect();
    Ok((
        MailResponse::Messages {
            messages,
            unread_left: unread_for(connection, key)?,
        },
        false,
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
        let tabs =
            brachio_store::task_tabs(connection, &task.id).map_err(|error| error.to_string())?;
        for tab in tabs
            .into_iter()
            .filter(|tab| tab.kind != brachio_store::TabKind::Shell)
        {
            peers.push(Peer {
                key: format!("{}:agent-{}", task.id, tab.tab_id),
                title: format!("{} (extra agent)", task.title),
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

fn list_tasks(connection: &Connection, key: &str) -> Outcome {
    let lead = agent_owner(connection, key)?;
    let children = child_reports(connection, &lead.id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|child| ChildTaskInfo {
            key: format!("{}:agent", child.task_id),
            title: child.title,
            archived: child.archived,
            reported: child.reported_subject.is_some(),
            unread: child.unread,
        })
        .collect();
    Ok((MailResponse::TaskList { children }, false))
}

/// Which open children of the lead have reported and which have not, right now.
fn child_status(connection: &Connection, key: &str) -> Outcome {
    let lead = agent_owner(connection, key)?;
    let mut reported = Vec::new();
    let mut pending = Vec::new();
    for child in child_reports(connection, &lead.id).map_err(|error| error.to_string())? {
        if child.archived {
            continue;
        }
        let key = format!("{}:agent", child.task_id);
        match child.reported_subject {
            Some(subject) => reported.push(ReportedChild {
                key,
                subject: subject.chars().take(REPORT_SUBJECT_CHARS).collect(),
            }),
            None => pending.push(key),
        }
    }
    Ok((
        MailResponse::TasksReported {
            reported,
            pending,
            timed_out: false,
        },
        false,
    ))
}

/// Looks at `poll` every `interval` until the children have reported or `timeout` passes. Dropping
/// the future stops the wait.
pub async fn wait_for_reports(
    mut poll: impl FnMut() -> MailResponse,
    all: bool,
    timeout: Duration,
    interval: Duration,
) -> MailResponse {
    let deadline = tokio::time::Instant::now() + timeout.min(Duration::from_secs(MAX_WAIT_SECS));
    loop {
        let response = poll();
        let MailResponse::TasksReported {
            reported, pending, ..
        } = response
        else {
            return response;
        };
        let done = pending.is_empty() || (!all && !reported.is_empty());
        let now = tokio::time::Instant::now();
        if done || now >= deadline {
            return MailResponse::TasksReported {
                reported,
                pending,
                timed_out: !done,
            };
        }
        tokio::time::sleep(interval.min(deadline - now)).await;
    }
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
        peek_inbox(&connection, &tab_key, MAX_BATCH).map_err(|error| error.to_string())?;
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

/// Hides messages of one terminal from the inbox. The agent still receives them. With no ids,
/// hides every message the inbox shows for the terminal.
#[tauri::command]
pub fn mailbox_dismiss(
    app: AppHandle,
    database: State<'_, Database>,
    tab_key: String,
    ids: Option<Vec<String>>,
) -> Result<(), String> {
    let mut connection = database.lock()?;
    dismiss_messages(
        &mut connection,
        &tab_key,
        ids.as_deref(),
        crate::pr_status::now_ms(),
    )
    .map_err(|error| error.to_string())?;
    drop(connection);
    if let Err(error) = app.emit(CHANGED_EVENT, ()) {
        tracing::debug!(%error, "could not tell the window about dismissed mail");
    }
    Ok(())
}

/// Marks messages read after the answer that carried them was written to the client.
pub fn mark_delivered(app: &AppHandle, ids: &[String]) {
    let database = app.state::<Database>();
    let Ok(mut connection) = database.lock() else {
        tracing::warn!("could not mark delivered messages read: database busy");
        return;
    };
    let marked = mark_read(&mut connection, ids, crate::pr_status::now_ms());
    drop(connection);
    match marked {
        Ok(0) => {}
        Ok(_) => {
            if let Err(error) = app.emit(CHANGED_EVENT, ()) {
                tracing::debug!(%error, "could not tell the window about read mail");
            }
        }
        Err(error) => tracing::warn!(%error, "could not mark delivered messages read"),
    }
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
    if matches!(response, MailResponse::HandedOff { .. }) {
        if let Err(error) = app.emit(crate::orchestration::CHANGED_EVENT, ()) {
            tracing::debug!(%error, "could not tell the window about a handoff");
        }
        crate::orchestration::schedule_all(app);
    }
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

    use std::time::Duration;

    use super::{handle, wait_for_reports};

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
        assert!(!changed);
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
        assert_eq!(unread_left, 1);
        let (again, _) = check(&mut connection);
        assert!(
            matches!(again, MailResponse::Messages { ref messages, .. } if messages.len() == 1)
        );
        brachio_store::mark_read(&mut connection, &["m1"], 25).expect("mark");
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
    fn inferred_reply_is_delivered_to_the_other_participant_in_the_same_thread() {
        let mut connection = database();
        let (_, changed) = handle(
            &mut connection,
            mail("aaaa1111:agent", "bbbb2222:agent", "Question"),
            1,
            "m1",
        );
        assert!(changed);
        let MailRequest::Send(mut reply) = mail("bbbb2222:agent", "", "Answer") else {
            panic!("expected a send request");
        };
        reply.reply_to = Some("m1".into());
        let (response, changed) =
            handle(&mut connection, MailRequest::Send(reply.clone()), 2, "m2");
        assert!(changed);
        assert!(matches!(response, MailResponse::Sent { thread_id, to, .. }
            if thread_id == "m1" && to == "aaaa1111:agent"));

        reply.reply_to = Some("unknown".into());
        let (response, changed) = handle(&mut connection, MailRequest::Send(reply), 3, "m3");
        assert!(!changed);
        assert!(matches!(response, MailResponse::Error { .. }));
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
    fn an_extra_agent_tab_is_listed_and_can_receive_mail() {
        let mut connection = database();
        let tab = "123e4567-e89b-12d3-a456-426614174000";
        connection
            .execute(
                "INSERT INTO task_tabs (task_id, tab_id, kind) VALUES ('bbbb2222', ?1, 'codex')",
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
        assert_eq!(second.title, "Write docs (extra agent)");
        assert_eq!(second.branch, "b2");

        let (answer, changed) =
            handle(&mut connection, mail("aaaa1111:agent", &key, "hi"), 2, "m1");
        assert!(changed);
        assert!(matches!(answer, MailResponse::Sent { ref to, .. } if *to == key));
    }

    fn with_children() -> rusqlite::Connection {
        let connection = database();
        connection
            .execute_batch(
                "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, parent_task_id)
                 VALUES ('cccc3333', 'r', 'Child one', '', 'b3', '/w3', 'new', 5, 'aaaa1111'),
                        ('dddd4444', 'r', 'Child two', '', 'b4', '/w4', 'new', 6, 'aaaa1111');",
            )
            .expect("insert children");
        connection
    }

    fn done(connection: &mut rusqlite::Connection, child: &str, subject: &str) {
        let from = format!("{child}:agent");
        let MailRequest::Send(mut report) = mail(&from, "parent", subject) else {
            panic!("expected a send request");
        };
        report.kind = "worker_done".into();
        let (answer, _) = handle(connection, MailRequest::Send(report), 7, subject);
        assert!(
            matches!(answer, MailResponse::Sent { ref to, .. } if to == "aaaa1111:agent"),
            "{answer:?}"
        );
    }

    #[test]
    fn to_parent_reaches_the_parent_agent_and_a_task_without_parent_is_told_so() {
        let mut connection = with_children();
        done(&mut connection, "cccc3333", "done one");
        let (answer, _) = handle(
            &mut connection,
            mail("aaaa1111:agent", "parent", "x"),
            8,
            "m",
        );
        assert!(
            matches!(answer, MailResponse::Error { ref message } if message == "this task has no parent")
        );
    }

    #[test]
    fn listing_shows_children_in_order_with_reported_and_unread() {
        let mut connection = with_children();
        done(&mut connection, "dddd4444", "done two");
        let (answer, _) = handle(
            &mut connection,
            MailRequest::ListTasks {
                key: "aaaa1111:agent".into(),
            },
            9,
            "x",
        );
        let MailResponse::TaskList { children } = answer else {
            panic!("expected a list");
        };
        let shown: Vec<_> = children
            .iter()
            .map(|c| (c.key.as_str(), c.reported, c.unread))
            .collect();
        assert_eq!(
            shown,
            [("cccc3333:agent", false, 0), ("dddd4444:agent", true, 1)]
        );
        let (refused, _) = handle(
            &mut connection,
            MailRequest::ListTasks {
                key: "aaaa1111:shell".into(),
            },
            9,
            "x",
        );
        assert!(matches!(refused, MailResponse::Error { .. }));
    }

    fn status(connection: &mut rusqlite::Connection) -> MailResponse {
        handle(
            connection,
            MailRequest::WaitTasks {
                key: "aaaa1111:agent".into(),
                all: true,
                timeout_secs: 1,
            },
            9,
            "x",
        )
        .0
    }

    fn run(
        mut poll: impl FnMut() -> MailResponse,
        all: bool,
        timeout_ms: u64,
    ) -> (MailResponse, u32) {
        let mut polls = 0;
        let response = tauri::async_runtime::block_on(wait_for_reports(
            || {
                polls += 1;
                poll()
            },
            all,
            Duration::from_millis(timeout_ms),
            Duration::from_millis(5),
        ));
        (response, polls)
    }

    fn reported_keys(response: &MailResponse) -> (Vec<&str>, Vec<&str>, bool) {
        let MailResponse::TasksReported {
            reported,
            pending,
            timed_out,
        } = response
        else {
            panic!("expected a report, got {response:?}");
        };
        (
            reported.iter().map(|r| r.key.as_str()).collect(),
            pending.iter().map(String::as_str).collect(),
            *timed_out,
        )
    }

    #[test]
    fn a_lead_without_open_children_does_not_wait() {
        let mut connection = database();
        let (response, polls) = run(|| status(&mut connection), true, 5_000);
        assert_eq!(polls, 1);
        assert_eq!(reported_keys(&response), (vec![], vec![], false));
    }

    #[test]
    fn all_waits_for_every_child_and_any_for_the_first() {
        let mut connection = with_children();
        done(&mut connection, "cccc3333", "done one");
        let (response, polls) = run(|| status(&mut connection), false, 5_000);
        assert_eq!(polls, 1, "any returns at once because one has reported");
        assert_eq!(
            reported_keys(&response),
            (vec!["cccc3333:agent"], vec!["dddd4444:agent"], false)
        );

        let mut ticks = 0;
        let mut connection = with_children();
        done(&mut connection, "cccc3333", "done one");
        let shared = std::cell::RefCell::new(connection);
        let (response, polls) = run(
            || {
                ticks += 1;
                if ticks == 3 {
                    done(&mut shared.borrow_mut(), "dddd4444", "done two");
                }
                status(&mut shared.borrow_mut())
            },
            true,
            5_000,
        );
        assert_eq!(polls, 3, "all keeps waiting until the second child reports");
        assert_eq!(
            reported_keys(&response),
            (vec!["cccc3333:agent", "dddd4444:agent"], vec![], false)
        );
    }

    #[test]
    fn running_out_of_time_returns_the_children_still_pending() {
        let mut connection = with_children();
        done(&mut connection, "cccc3333", "done one");
        let (response, polls) = run(|| status(&mut connection), true, 30);
        assert!(polls > 1);
        assert_eq!(
            reported_keys(&response),
            (vec!["cccc3333:agent"], vec!["dddd4444:agent"], true)
        );
    }

    #[test]
    fn archived_children_are_not_waited_for() {
        let mut connection = with_children();
        connection
            .execute(
                "UPDATE tasks SET archived_at_ms = 1 WHERE id = 'dddd4444'",
                [],
            )
            .expect("archive");
        done(&mut connection, "cccc3333", "done one");
        let (response, _) = run(|| status(&mut connection), true, 5_000);
        assert_eq!(
            reported_keys(&response),
            (vec!["cccc3333:agent"], vec![], false)
        );
    }
}
