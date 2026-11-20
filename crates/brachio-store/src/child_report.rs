//! What a lead agent sees of its child tasks: which exist, which reported done, how much mail
//! they left unread.

use rusqlite::{params, Connection};

use crate::StoreError;

/// A child task of a lead, and whether its agent has reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildReport {
    /// The child's task id.
    pub task_id: String,
    /// The child's title.
    pub title: String,
    /// Archived children no longer count as pending work.
    pub archived: bool,
    /// Subject of the child agent's latest `worker_done` message to the lead, if any.
    pub reported_subject: Option<String>,
    /// Messages from the child's agent to the lead that the lead has not read.
    pub unread: u32,
}

/// The children of `lead_task_id` in creation order. Only messages from the child's own agent to
/// the lead's agent count, so mail the lead got before the child existed never shows up here.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn child_reports(
    connection: &Connection,
    lead_task_id: &str,
) -> Result<Vec<ChildReport>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT t.id, t.title, t.archived_at_ms IS NOT NULL,
                (SELECT m.subject FROM messages m
                  WHERE m.from_key = t.id || ':agent' AND m.to_key = ?1 || ':agent'
                    AND m.kind = 'worker_done'
                  ORDER BY m.created_at_ms DESC, m.rowid DESC LIMIT 1),
                (SELECT COUNT(*) FROM messages m
                  WHERE m.from_key = t.id || ':agent' AND m.to_key = ?1 || ':agent'
                    AND m.read_at_ms IS NULL)
           FROM tasks t WHERE t.parent_task_id = ?1
          ORDER BY t.created_at_ms, t.rowid",
    )?;
    let rows = statement.query_map(params![lead_task_id], |row| {
        Ok(ChildReport {
            task_id: row.get(0)?,
            title: row.get(1)?,
            archived: row.get(2)?,
            reported_subject: row.get(3)?,
            unread: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{open_in_memory, send_message, take_unread, NewMessage};

    fn database() -> Connection {
        let connection = open_in_memory().expect("open");
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, parent_task_id, archived_at_ms)
                 VALUES ('lead', 'r', 'Lead', '', 'b0', '/w0', 'new', 0, NULL, NULL),
                        ('other', 'r', 'Other lead', '', 'b9', '/w9', 'new', 0, NULL, NULL),
                        ('c2', 'r', 'Second', '', 'b2', '/w2', 'new', 20, 'lead', NULL),
                        ('c1', 'r', 'First', '', 'b1', '/w1', 'new', 10, 'lead', NULL),
                        ('c3', 'r', 'Gone', '', 'b3', '/w3', 'new', 30, 'lead', 99),
                        ('o1', 'r', 'Foreign child', '', 'b4', '/w4', 'new', 40, 'other', NULL);",
            )
            .expect("insert tasks");
        connection
    }

    fn send(connection: &Connection, id: &str, from: &str, to: &str, kind: &str, subject: &str) {
        send_message(
            connection,
            &NewMessage {
                id,
                from_key: from,
                to_key: to,
                kind,
                priority: "normal",
                subject,
                body: "",
                payload_json: None,
                reply_to: None,
                request_id: None,
                created_at_ms: 1,
            },
        )
        .expect("send");
    }

    #[test]
    fn children_come_in_creation_order_and_only_those_that_sent_worker_done_are_reported() {
        let connection = database();
        send(
            &connection,
            "m1",
            "c1:agent",
            "lead:agent",
            "worker_done",
            "done: parser",
        );
        send(
            &connection,
            "m2",
            "c2:agent",
            "lead:agent",
            "status",
            "halfway",
        );

        let reports = child_reports(&connection, "lead").expect("reports");
        let ids: Vec<_> = reports.iter().map(|r| r.task_id.as_str()).collect();
        assert_eq!(ids, ["c1", "c2", "c3"]);
        assert_eq!(reports[0].reported_subject.as_deref(), Some("done: parser"));
        assert_eq!(reports[1].reported_subject, None);
        assert_eq!(reports[1].unread, 1);
        assert!(reports[2].archived && !reports[0].archived);
    }

    #[test]
    fn worker_done_from_strangers_or_to_other_leads_does_not_count() {
        let connection = database();
        send(
            &connection,
            "m1",
            "o1:agent",
            "lead:agent",
            "worker_done",
            "not mine",
        );
        send(
            &connection,
            "m2",
            "other:agent",
            "lead:agent",
            "worker_done",
            "not a child",
        );
        send(
            &connection,
            "m3",
            "c1:agent",
            "other:agent",
            "worker_done",
            "wrong lead",
        );
        send(
            &connection,
            "m4",
            "c1:agent",
            "lead:shell",
            "worker_done",
            "wrong terminal",
        );

        let reports = child_reports(&connection, "lead").expect("reports");
        assert!(reports.iter().all(|r| r.reported_subject.is_none()));
        assert!(reports.iter().all(|r| r.unread == 0));
        assert_eq!(child_reports(&connection, "other").expect("other").len(), 1);
    }

    #[test]
    fn a_report_stays_after_the_lead_has_read_it_but_stops_being_unread() {
        let mut connection = database();
        send(
            &connection,
            "m1",
            "c1:agent",
            "lead:agent",
            "worker_done",
            "done",
        );
        take_unread(&mut connection, "lead:agent", 10, 5).expect("read");
        let reports = child_reports(&connection, "lead").expect("reports");
        assert_eq!(reports[0].reported_subject.as_deref(), Some("done"));
        assert_eq!(reports[0].unread, 0);
    }
}
