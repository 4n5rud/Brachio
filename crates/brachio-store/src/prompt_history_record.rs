//! Prompts sent through Brachio, kept per project and recalled in the new-task field.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::StoreError;

/// Most entries kept for one project; older ones are dropped.
pub const MAX_PROMPT_HISTORY: i64 = 200;
/// Longest prompt kept.
pub const MAX_HISTORY_BODY_CHARS: usize = 20_000;

/// One remembered prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptHistoryEntry {
    /// Ordered id; a larger one is newer.
    pub id: i64,
    /// The task it was sent to, when there was one.
    pub task_id: Option<String>,
    /// `prompt`, `cue` or `draft`.
    pub kind: String,
    /// The text.
    pub body: String,
}

/// Remembers a prompt, skipping one that repeats the newest entry, and trims the project to the cap.
///
/// # Errors
/// Fails for an unknown kind, an empty or too long text, an unknown project, or a database error.
pub fn add_prompt_history(
    connection: &Connection,
    repo_id: &str,
    task_id: Option<&str>,
    kind: &str,
    body: &str,
    at_ms: i64,
) -> Result<(), StoreError> {
    let refuse = |reason: &str| StoreError::Invalid(reason.to_owned());
    if !matches!(kind, "prompt" | "cue" | "draft") {
        return Err(refuse("unknown history kind"));
    }
    let body = body.trim();
    if body.is_empty() {
        return Err(refuse("nothing to remember"));
    }
    if body.chars().count() > MAX_HISTORY_BODY_CHARS {
        return Err(refuse("the prompt is too long"));
    }
    let newest: Option<String> = connection
        .query_row(
            "SELECT body FROM prompt_history WHERE repo_id = ?1 ORDER BY id DESC LIMIT 1",
            [repo_id],
            |row| row.get(0),
        )
        .ok();
    if newest.as_deref() == Some(body) {
        return Ok(());
    }
    connection.execute(
        "INSERT INTO prompt_history (repo_id, task_id, kind, body, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![repo_id, task_id, kind, body, at_ms],
    )?;
    connection.execute(
        "DELETE FROM prompt_history WHERE repo_id = ?1 AND id NOT IN
         (SELECT id FROM prompt_history WHERE repo_id = ?1 ORDER BY id DESC LIMIT ?2)",
        params![repo_id, MAX_PROMPT_HISTORY],
    )?;
    Ok(())
}

/// The project's history, newest first.
///
/// # Errors
/// Fails when the query fails.
pub fn prompt_history(
    connection: &Connection,
    repo_id: &str,
) -> Result<Vec<PromptHistoryEntry>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT id, task_id, kind, body FROM prompt_history WHERE repo_id = ?1 ORDER BY id DESC",
    )?;
    let rows = statement.query_map([repo_id], |row| {
        Ok(PromptHistoryEntry {
            id: row.get(0)?,
            task_id: row.get(1)?,
            kind: row.get(2)?,
            body: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{open_in_memory, NewRepo, RepoRecord};
    use std::path::Path;

    fn connection_with_repo() -> Connection {
        let connection = open_in_memory().expect("open");
        RepoRecord::insert(
            &connection,
            &NewRepo {
                id: "r",
                name: "r",
                path: Path::new("/r"),
                default_branch: "main",
                added_at_ms: 0,
                is_git: true,
            },
        )
        .expect("repo");
        connection
    }

    #[test]
    fn history_is_newest_first_and_skips_an_immediate_repeat() {
        let connection = connection_with_repo();
        for body in ["one", "two", "two", "three"] {
            add_prompt_history(&connection, "r", None, "prompt", body, 1).expect("add");
        }
        let bodies: Vec<_> = prompt_history(&connection, "r")
            .expect("list")
            .into_iter()
            .map(|entry| entry.body)
            .collect();
        assert_eq!(bodies, ["three", "two", "one"]);
    }

    #[test]
    fn a_project_keeps_only_the_newest_two_hundred() {
        let connection = connection_with_repo();
        for index in 0..205 {
            add_prompt_history(&connection, "r", None, "prompt", &format!("p{index}"), 1)
                .expect("add");
        }
        let entries = prompt_history(&connection, "r").expect("list");
        assert_eq!(entries.len(), 200);
        assert_eq!(entries[0].body, "p204");
        assert_eq!(entries[199].body, "p5");
    }

    #[test]
    fn empty_text_and_unknown_kinds_are_refused() {
        let connection = connection_with_repo();
        assert!(add_prompt_history(&connection, "r", None, "prompt", "  ", 1).is_err());
        assert!(add_prompt_history(&connection, "r", None, "other", "x", 1).is_err());
    }
}
