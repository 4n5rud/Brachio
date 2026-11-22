//! Decision gates: a child asks the user to choose before the tasks that depend on it go on.
//! A gate is a `decision_gate` message whose payload lists the options; it stays open until a
//! reply to it exists, and only the user's answer writes that reply.

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

/// Most options one gate may offer.
pub const MAX_GATE_OPTIONS: usize = 6;
/// Longest option, in characters.
pub const MAX_GATE_OPTION_CHARS: usize = 80;

/// A gate nobody has answered yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingGate {
    /// The `decision_gate` message.
    pub message_id: String,
    /// The child that raised it.
    pub task_id: String,
    /// One line saying what must be decided.
    pub subject: String,
    /// Details; untrusted.
    pub body: String,
    /// The choices the child supplied.
    pub options: Vec<String>,
    /// Raised at, in milliseconds since the Unix epoch.
    pub created_at_ms: i64,
}

/// Reads the options out of a gate's payload, `{"options": ["a", "b"]}`.
///
/// # Errors
/// A message for the sender when the payload is missing, malformed, or offers fewer than two
/// or more than [`MAX_GATE_OPTIONS`] distinct, printable options.
pub fn gate_options(payload: Option<&str>) -> Result<Vec<String>, String> {
    let wrong = || {
        format!(
            "A decision gate needs 2 to {MAX_GATE_OPTIONS} options of at most \
             {MAX_GATE_OPTION_CHARS} printable characters each."
        )
    };
    let value: Value = serde_json::from_str(payload.ok_or_else(wrong)?).map_err(|_| wrong())?;
    let items = value
        .get("options")
        .and_then(Value::as_array)
        .ok_or_else(wrong)?;
    let mut options: Vec<String> = Vec::new();
    for item in items {
        let text = item.as_str().ok_or_else(wrong)?.trim();
        if text.is_empty()
            || text.chars().count() > MAX_GATE_OPTION_CHARS
            || text.chars().any(char::is_control)
            || options.iter().any(|seen| seen == text)
        {
            return Err(wrong());
        }
        options.push(text.to_owned());
    }
    if options.len() < 2 || options.len() > MAX_GATE_OPTIONS {
        return Err(wrong());
    }
    Ok(options)
}

const GATE_SELECT: &str =
    "SELECT m.id, m.from_key, m.subject, m.body, m.payload_json, m.created_at_ms
     FROM messages m
     WHERE m.kind = 'decision_gate'
       AND NOT EXISTS (SELECT 1 FROM messages r WHERE r.reply_to = m.id)";

fn gate_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<PendingGate>> {
    let from_key: String = row.get(1)?;
    let payload: Option<String> = row.get(4)?;
    let Some(task_id) = from_key.strip_suffix(":agent") else {
        return Ok(None);
    };
    let Ok(options) = gate_options(payload.as_deref()) else {
        return Ok(None);
    };
    Ok(Some(PendingGate {
        message_id: row.get(0)?,
        task_id: task_id.to_owned(),
        subject: row.get(2)?,
        body: row.get(3)?,
        options,
        created_at_ms: row.get(5)?,
    }))
}

/// The open gates of one child, oldest first.
///
/// # Errors
/// Fails when the database fails.
pub fn pending_gates(
    connection: &Connection,
    task_id: &str,
) -> Result<Vec<PendingGate>, rusqlite::Error> {
    let mut statement = connection.prepare(&format!(
        "{GATE_SELECT} AND m.from_key = ?1 ORDER BY m.created_at_ms, m.id"
    ))?;
    let rows = statement.query_map(params![format!("{task_id}:agent")], gate_from)?;
    let mut gates = Vec::new();
    for row in rows {
        gates.extend(row?);
    }
    Ok(gates)
}

/// One open gate by its message id; `None` when it does not exist or was answered.
///
/// # Errors
/// Fails when the database fails.
pub fn pending_gate(
    connection: &Connection,
    message_id: &str,
) -> Result<Option<PendingGate>, rusqlite::Error> {
    Ok(connection
        .query_row(
            &format!("{GATE_SELECT} AND m.id = ?1"),
            params![message_id],
            gate_from,
        )
        .optional()?
        .flatten())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{open_in_memory, send_message, NewMessage};

    fn send(
        connection: &Connection,
        id: &str,
        kind: &str,
        reply_to: Option<&str>,
        payload: Option<&str>,
    ) {
        send_message(
            connection,
            &NewMessage {
                id,
                from_key: if reply_to.is_some() {
                    "lead:agent"
                } else {
                    "c1:agent"
                },
                to_key: if reply_to.is_some() {
                    "c1:agent"
                } else {
                    "lead:agent"
                },
                kind,
                priority: "normal",
                subject: "Pick one",
                body: "",
                payload_json: payload,
                reply_to,
                request_id: None,
                created_at_ms: 1,
            },
        )
        .unwrap();
    }

    #[test]
    fn a_gate_stays_open_until_a_reply_exists() {
        let connection = open_in_memory().unwrap();
        send(
            &connection,
            "g1",
            "decision_gate",
            None,
            Some(r#"{"options":["A","B"]}"#),
        );
        let gates = pending_gates(&connection, "c1").unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].options, ["A", "B"]);
        send(&connection, "r1", "status", Some("g1"), None);
        assert!(pending_gates(&connection, "c1").unwrap().is_empty());
        assert!(pending_gate(&connection, "g1").unwrap().is_none());
    }

    #[test]
    fn options_must_be_two_to_six_distinct_printable_choices() {
        assert!(gate_options(Some(r#"{"options":["A","B"]}"#)).is_ok());
        for bad in [
            None,
            Some("not json"),
            Some(r#"{"options":["A"]}"#),
            Some(r#"{"options":["A","A"]}"#),
            Some(r#"{"options":["A","B\nC"]}"#),
            Some(r#"{"options":["1","2","3","4","5","6","7"]}"#),
        ] {
            assert!(gate_options(bad).is_err(), "{bad:?}");
        }
    }
}
