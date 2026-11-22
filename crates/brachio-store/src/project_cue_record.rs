//! Reusable prompts and shell commands kept per project.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::StoreError;

/// Longest label a cue may have.
pub const MAX_CUE_LABEL_CHARS: usize = 80;
/// Longest prompt or command a cue may hold.
pub const MAX_CUE_BODY_CHARS: usize = 4000;

/// One quick action of a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCue {
    /// Unique id.
    pub id: String,
    /// The project it belongs to.
    pub repo_id: String,
    /// `prompt` is typed into the agent; `shell` is run in a shell.
    pub kind: String,
    /// Short name shown in menus.
    pub label: String,
    /// The text of the prompt or the command.
    pub body: String,
}

/// Adds a cue after checking its kind and lengths.
///
/// # Errors
/// Fails for an unknown project, an empty or too long label or body, an unknown kind, or when the
/// database call fails.
pub fn add_project_cue(
    connection: &Connection,
    cue: &ProjectCue,
    at_ms: i64,
) -> Result<(), StoreError> {
    let refuse = |reason: &str| StoreError::Invalid(reason.to_owned());
    if cue.kind != "prompt" && cue.kind != "shell" {
        return Err(refuse("a cue is a prompt or a shell command"));
    }
    if cue.label.trim().is_empty() || cue.body.trim().is_empty() {
        return Err(refuse("a cue needs a name and text"));
    }
    if cue.label.chars().count() > MAX_CUE_LABEL_CHARS
        || cue.body.chars().count() > MAX_CUE_BODY_CHARS
    {
        return Err(refuse("the cue is too long"));
    }
    connection.execute(
        "INSERT INTO project_cues (id, repo_id, kind, label, body, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![cue.id, cue.repo_id, cue.kind, cue.label.trim(), cue.body, at_ms],
    )?;
    Ok(())
}

/// Removes a cue; an unknown id is not an error.
///
/// # Errors
/// Fails when the database call fails.
pub fn delete_project_cue(connection: &Connection, id: &str) -> Result<(), StoreError> {
    connection.execute("DELETE FROM project_cues WHERE id = ?1", [id])?;
    Ok(())
}

/// The cues of one project, oldest first.
///
/// # Errors
/// Fails when the query fails.
pub fn project_cues(connection: &Connection, repo_id: &str) -> Result<Vec<ProjectCue>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT id, repo_id, kind, label, body FROM project_cues
         WHERE repo_id = ?1 ORDER BY created_at_ms, id",
    )?;
    let rows = statement.query_map([repo_id], |row| {
        Ok(ProjectCue {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            kind: row.get(2)?,
            label: row.get(3)?,
            body: row.get(4)?,
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

    fn cue(id: &str, kind: &str) -> ProjectCue {
        ProjectCue {
            id: id.to_owned(),
            repo_id: "r".to_owned(),
            kind: kind.to_owned(),
            label: "Run tests".to_owned(),
            body: "pnpm test".to_owned(),
        }
    }

    #[test]
    fn cues_come_back_oldest_first_and_can_be_deleted() {
        let connection = connection_with_repo();
        add_project_cue(&connection, &cue("b", "shell"), 2).expect("add");
        add_project_cue(&connection, &cue("a", "prompt"), 1).expect("add");
        let ids: Vec<_> = project_cues(&connection, "r")
            .expect("list")
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids, ["a", "b"]);
        delete_project_cue(&connection, "a").expect("delete");
        assert_eq!(project_cues(&connection, "r").expect("list").len(), 1);
    }

    #[test]
    fn an_unknown_kind_or_empty_text_is_refused() {
        let connection = connection_with_repo();
        assert!(add_project_cue(&connection, &cue("a", "other"), 1).is_err());
        let mut empty = cue("b", "shell");
        empty.body = "  ".to_owned();
        assert!(add_project_cue(&connection, &empty, 1).is_err());
    }

    #[test]
    fn removing_a_project_removes_its_cues() {
        let connection = connection_with_repo();
        add_project_cue(&connection, &cue("a", "shell"), 1).expect("add");
        connection
            .execute("DELETE FROM repos WHERE id = 'r'", [])
            .expect("delete");
        assert!(project_cues(&connection, "r").expect("list").is_empty());
    }
}
