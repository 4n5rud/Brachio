//! The last pull request status read for a task, kept so it can be shown again after a restart
//! or while GitHub cannot be reached. The store does not look inside the snapshot.

use rusqlite::{params, Connection, OptionalExtension};

use crate::StoreError;

/// Saves the snapshot, replacing the previous one.
///
/// # Errors
/// Fails when the task does not exist or the database cannot be written.
pub fn save_pr_status(
    connection: &Connection,
    task_id: &str,
    snapshot_json: &str,
    checked_at_ms: i64,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO task_pr_status (task_id, snapshot_json, checked_at_ms)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(task_id) DO UPDATE SET
             snapshot_json = excluded.snapshot_json,
             checked_at_ms = excluded.checked_at_ms",
        params![task_id, snapshot_json, checked_at_ms],
    )?;
    Ok(())
}

/// Drops the saved snapshot, for a branch whose pull request is gone.
///
/// # Errors
/// Fails when the database cannot be written.
pub fn forget_pr_status(connection: &Connection, task_id: &str) -> Result<(), StoreError> {
    connection.execute("DELETE FROM task_pr_status WHERE task_id = ?1", [task_id])?;
    Ok(())
}

/// The saved snapshot and when it was read, if any.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn load_pr_status(
    connection: &Connection,
    task_id: &str,
) -> Result<Option<(String, i64)>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT snapshot_json, checked_at_ms FROM task_pr_status WHERE task_id = ?1",
            [task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_in_memory;

    fn with_task() -> Connection {
        let connection = open_in_memory().expect("open database");
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);",
            )
            .expect("insert a repo and a task");
        connection
    }

    #[test]
    fn the_latest_snapshot_is_kept_and_goes_with_its_task() {
        let connection = with_task();
        assert_eq!(load_pr_status(&connection, "t").expect("load"), None);
        save_pr_status(&connection, "t", "{\"n\":1}", 10).expect("save");
        save_pr_status(&connection, "t", "{\"n\":2}", 20).expect("save again");
        assert_eq!(
            load_pr_status(&connection, "t").expect("load"),
            Some(("{\"n\":2}".to_owned(), 20))
        );
        connection
            .execute("DELETE FROM tasks WHERE id = 't'", [])
            .expect("delete task");
        assert_eq!(load_pr_status(&connection, "t").expect("load"), None);
    }

    #[test]
    fn a_snapshot_can_be_dropped_on_its_own() {
        let connection = with_task();
        save_pr_status(&connection, "t", "{}", 1).expect("save");
        forget_pr_status(&connection, "t").expect("forget");
        assert_eq!(load_pr_status(&connection, "t").expect("load"), None);
        forget_pr_status(&connection, "t").expect("forgetting nothing is fine");
    }
}
