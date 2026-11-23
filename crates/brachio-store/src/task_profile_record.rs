//! The agent profile a task was created with, and joining a task to a comparison group.

use rusqlite::{params, Connection, OptionalExtension};

use crate::StoreError;

/// Remembers which `brachio.toml` profile the task uses.
///
/// # Errors
/// Fails when the task does not exist or the database fails.
pub fn set_task_profile(
    connection: &Connection,
    task_id: &str,
    profile: &str,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO task_profile (task_id, profile) VALUES (?1, ?2)
         ON CONFLICT(task_id) DO UPDATE SET profile = excluded.profile",
        params![task_id, profile],
    )?;
    Ok(())
}

/// The profile name of a task, if it has one.
///
/// # Errors
/// Fails when the database fails.
pub fn task_profile(connection: &Connection, task_id: &str) -> Result<Option<String>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT profile FROM task_profile WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Puts a task in a comparison group when it is in none. Returns the group the task is in.
///
/// # Errors
/// Fails when the task does not exist or the database fails.
pub fn join_compare_group(
    connection: &Connection,
    task_id: &str,
    group: &str,
) -> Result<String, StoreError> {
    connection.execute(
        "UPDATE tasks SET compare_group = ?2 WHERE id = ?1 AND compare_group IS NULL",
        params![task_id, group],
    )?;
    connection
        .query_row(
            "SELECT compare_group FROM tasks WHERE id = ?1",
            [task_id],
            |row| row.get::<_, Option<String>>(0),
        )?
        .ok_or_else(|| StoreError::NotFound {
            table: "tasks",
            id: task_id.to_owned(),
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let connection = crate::open_in_memory().unwrap();
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('a', 'r', 'A', 'p', 'a', '/w', 'new', 0);",
            )
            .unwrap();
        connection
    }

    #[test]
    fn a_task_remembers_its_profile() {
        let db = db();
        assert_eq!(task_profile(&db, "a").unwrap(), None);
        set_task_profile(&db, "a", "careful").unwrap();
        assert_eq!(task_profile(&db, "a").unwrap().as_deref(), Some("careful"));
    }

    #[test]
    fn a_task_keeps_the_group_it_already_has() {
        let db = db();
        assert_eq!(join_compare_group(&db, "a", "g1").unwrap(), "g1");
        assert_eq!(join_compare_group(&db, "a", "g2").unwrap(), "g1");
        assert!(join_compare_group(&db, "missing", "g1").is_err());
    }
}
