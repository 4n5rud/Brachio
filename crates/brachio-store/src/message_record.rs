//! Messages between agent sessions. A message waits here until its receiver asks for it, so
//! nothing is ever typed into an agent's context on its own.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::StoreError;

/// Longest body, in bytes. Anything bigger belongs in a file the message points to.
pub const MAX_BODY_BYTES: usize = 8 * 1024;

/// What a message is for.
pub const KINDS: [&str; 9] = [
    "status",
    "dispatch",
    "worker_done",
    "merge_ready",
    "escalation",
    "handoff",
    "decision_gate",
    "question",
    "heartbeat",
];

/// A message to store.
#[derive(Debug, Clone)]
pub struct NewMessage<'a> {
    /// Identifier of the message.
    pub id: &'a str,
    /// Sender's terminal key, such as `<task id>:agent`.
    pub from_key: &'a str,
    /// Receiver's terminal key.
    pub to_key: &'a str,
    /// One of [`KINDS`].
    pub kind: &'a str,
    /// `low`, `normal` or `high`.
    pub priority: &'a str,
    /// One line saying what this is about.
    pub subject: &'a str,
    /// Details, at most [`MAX_BODY_BYTES`].
    pub body: &'a str,
    /// Structured references such as file paths, as JSON text.
    pub payload_json: Option<&'a str>,
    /// The message this answers.
    pub reply_to: Option<&'a str>,
    /// Sending the same request id again returns the first message instead of a second one.
    pub request_id: Option<&'a str>,
    /// When it was sent, in milliseconds since the Unix epoch.
    pub created_at_ms: i64,
}

/// A stored message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Identifier.
    pub id: String,
    /// The conversation it belongs to; a reply keeps its parent's.
    pub thread_id: String,
    /// The message this answers.
    pub reply_to: Option<String>,
    /// Sender's terminal key.
    pub from_key: String,
    /// Receiver's terminal key.
    pub to_key: String,
    /// One of [`KINDS`].
    pub kind: String,
    /// `low`, `normal` or `high`.
    pub priority: String,
    /// One line.
    pub subject: String,
    /// Details.
    pub body: String,
    /// Structured references, as JSON text.
    pub payload_json: Option<String>,
    /// Sent at, in milliseconds since the Unix epoch.
    pub created_at_ms: i64,
    /// Read at, or `None` while unread.
    pub read_at_ms: Option<i64>,
}

const COLUMNS: &str = "id, thread_id, reply_to, from_key, to_key, kind, priority, subject, body, \
                       payload_json, created_at_ms, read_at_ms";

fn message_from(row: &Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        reply_to: row.get(2)?,
        from_key: row.get(3)?,
        to_key: row.get(4)?,
        kind: row.get(5)?,
        priority: row.get(6)?,
        subject: row.get(7)?,
        body: row.get(8)?,
        payload_json: row.get(9)?,
        created_at_ms: row.get(10)?,
        read_at_ms: row.get(11)?,
    })
}

/// Stores a message and returns it. A repeated request id returns the message stored the first
/// time, with `true` as the second value.
///
/// # Errors
/// Fails with [`StoreError::Invalid`] for an unknown kind, an empty subject or a body over
/// [`MAX_BODY_BYTES`], and with a database error otherwise.
pub fn send_message(
    connection: &Connection,
    message: &NewMessage<'_>,
) -> Result<(Message, bool), StoreError> {
    if !KINDS.contains(&message.kind) {
        return Err(StoreError::Invalid(format!(
            "unknown message type {:?}; use one of {}",
            message.kind,
            KINDS.join(", ")
        )));
    }
    if message.subject.trim().is_empty() {
        return Err(StoreError::Invalid("a message needs a subject".to_owned()));
    }
    if message.body.len() > MAX_BODY_BYTES {
        return Err(StoreError::Invalid(format!(
            "the body is {} bytes; the limit is {MAX_BODY_BYTES}. Put the details in a file and send its path.",
            message.body.len()
        )));
    }
    if let Some(request_id) = message.request_id {
        let earlier = connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM messages WHERE from_key = ?1 AND request_id = ?2"),
                params![message.from_key, request_id],
                message_from,
            )
            .optional()?;
        if let Some(earlier) = earlier {
            return Ok((earlier, true));
        }
    }
    let thread_id: String = match message.reply_to {
        Some(parent) => connection
            .query_row(
                "SELECT thread_id FROM messages WHERE id = ?1",
                [parent],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound {
                table: "messages",
                id: parent.to_owned(),
            })?,
        None => message.id.to_owned(),
    };
    connection.execute(
        "INSERT INTO messages (id, thread_id, reply_to, from_key, to_key, kind, priority, subject, \
                               body, payload_json, request_id, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            message.id,
            thread_id,
            message.reply_to,
            message.from_key,
            message.to_key,
            message.kind,
            message.priority,
            message.subject.trim(),
            message.body,
            message.payload_json,
            message.request_id,
            message.created_at_ms,
        ],
    )?;
    let stored = connection.query_row(
        &format!("SELECT {COLUMNS} FROM messages WHERE id = ?1"),
        [message.id],
        message_from,
    )?;
    Ok((stored, false))
}

/// The oldest unread messages for `to_key`, at most `limit`, marked as read.
///
/// # Errors
/// Fails when the database cannot be read or written.
pub fn take_unread(
    connection: &mut Connection,
    to_key: &str,
    limit: usize,
    now_ms: i64,
) -> Result<Vec<Message>, StoreError> {
    let transaction = connection.transaction()?;
    let batch = unread(&transaction, to_key, limit)?;
    for message in &batch {
        transaction.execute(
            "UPDATE messages SET read_at_ms = ?2 WHERE id = ?1",
            params![message.id, now_ms],
        )?;
    }
    transaction.commit()?;
    Ok(batch)
}

/// The oldest unread messages for `to_key`, left unread.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn peek_unread(
    connection: &Connection,
    to_key: &str,
    limit: usize,
) -> Result<Vec<Message>, StoreError> {
    unread(connection, to_key, limit)
}

fn unread(connection: &Connection, to_key: &str, limit: usize) -> Result<Vec<Message>, StoreError> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM messages WHERE to_key = ?1 AND read_at_ms IS NULL \
         ORDER BY created_at_ms, rowid LIMIT ?2"
    ))?;
    let rows = statement.query_map(
        params![to_key, i64::try_from(limit).unwrap_or(i64::MAX)],
        message_from,
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The latest messages to or from `key`, newest first, read or not.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn recent_messages(
    connection: &Connection,
    key: &str,
    limit: usize,
) -> Result<Vec<Message>, StoreError> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM messages WHERE to_key = ?1 OR from_key = ?1 \
         ORDER BY created_at_ms DESC, rowid DESC LIMIT ?2"
    ))?;
    let rows = statement.query_map(
        params![key, i64::try_from(limit).unwrap_or(i64::MAX)],
        message_from,
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// How many unread messages wait for each receiver.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn unread_counts(connection: &Connection) -> Result<Vec<(String, u32)>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT to_key, COUNT(*) FROM messages WHERE read_at_ms IS NULL \
         GROUP BY to_key ORDER BY to_key",
    )?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Removes every message to or from the terminals of a task.
///
/// # Errors
/// Fails when the database cannot be written.
pub fn forget_task_messages(connection: &Connection, task_id: &str) -> Result<(), StoreError> {
    let mut pattern = String::new();
    for character in task_id.chars() {
        if matches!(character, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push_str(":%");
    connection.execute(
        "DELETE FROM messages WHERE to_key LIKE ?1 ESCAPE '\\' OR from_key LIKE ?1 ESCAPE '\\'",
        [pattern],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_in_memory;

    fn message<'a>(id: &'a str, to: &'a str, at: i64) -> NewMessage<'a> {
        NewMessage {
            id,
            from_key: "a:agent",
            to_key: to,
            kind: "status",
            priority: "normal",
            subject: "done",
            body: "",
            payload_json: None,
            reply_to: None,
            request_id: None,
            created_at_ms: at,
        }
    }

    #[test]
    fn messages_are_read_oldest_first_and_only_once() {
        let mut connection = open_in_memory().expect("open");
        send_message(&connection, &message("m2", "b:agent", 20)).expect("send");
        send_message(&connection, &message("m1", "b:agent", 10)).expect("send");
        send_message(&connection, &message("m3", "c:agent", 5)).expect("send");

        assert_eq!(
            unread_counts(&connection).expect("counts"),
            vec![("b:agent".to_owned(), 2), ("c:agent".to_owned(), 1)]
        );
        let peeked = peek_unread(&connection, "b:agent", 10).expect("peek");
        assert_eq!(peeked.len(), 2);
        let taken = take_unread(&mut connection, "b:agent", 1, 99).expect("take");
        assert_eq!(taken[0].id, "m1");
        let rest = take_unread(&mut connection, "b:agent", 10, 100).expect("take");
        assert_eq!(
            rest.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["m2"]
        );
        assert!(take_unread(&mut connection, "b:agent", 10, 101)
            .expect("take")
            .is_empty());
    }

    #[test]
    fn sending_a_request_again_returns_the_first_message() {
        let connection = open_in_memory().expect("open");
        let mut first = message("m1", "b:agent", 1);
        first.request_id = Some("r1");
        let mut again = message("m2", "b:agent", 2);
        again.request_id = Some("r1");
        let (stored, duplicate) = send_message(&connection, &first).expect("send");
        assert!(!duplicate);
        let (second, duplicate) = send_message(&connection, &again).expect("send again");
        assert!(duplicate);
        assert_eq!(second.id, stored.id);
        assert_eq!(unread_counts(&connection).expect("counts")[0].1, 1);
    }

    #[test]
    fn a_reply_stays_in_the_thread_of_its_parent() {
        let connection = open_in_memory().expect("open");
        send_message(&connection, &message("m1", "b:agent", 1)).expect("send");
        let mut reply = message("m2", "a:agent", 2);
        reply.reply_to = Some("m1");
        let (stored, _) = send_message(&connection, &reply).expect("reply");
        assert_eq!(stored.thread_id, "m1");
        let mut orphan = message("m3", "a:agent", 3);
        orphan.reply_to = Some("missing");
        assert!(send_message(&connection, &orphan).is_err());
    }

    #[test]
    fn bad_type_empty_subject_and_a_huge_body_are_refused() {
        let connection = open_in_memory().expect("open");
        let mut bad_kind = message("m1", "b:agent", 1);
        bad_kind.kind = "gossip";
        assert!(send_message(&connection, &bad_kind).is_err());
        let mut no_subject = message("m2", "b:agent", 1);
        no_subject.subject = "  ";
        assert!(send_message(&connection, &no_subject).is_err());
        let huge = "x".repeat(MAX_BODY_BYTES + 1);
        let mut big = message("m3", "b:agent", 1);
        big.body = &huge;
        assert!(send_message(&connection, &big).is_err());
    }

    #[test]
    fn forgetting_a_task_removes_its_mail_in_both_directions_and_nobody_elses() {
        let connection = open_in_memory().expect("open");
        send_message(&connection, &message("m1", "t1:agent", 1)).expect("send");
        let mut from = message("m2", "t2:agent", 2);
        from.from_key = "t1:agent";
        send_message(&connection, &from).expect("send");
        send_message(&connection, &message("m3", "t2:agent", 3)).expect("send");
        forget_task_messages(&connection, "t1").expect("forget");
        let left = recent_messages(&connection, "t2:agent", 10).expect("recent");
        assert_eq!(
            left.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["m3"]
        );
    }
}
