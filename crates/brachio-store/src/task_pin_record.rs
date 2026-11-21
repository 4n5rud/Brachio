//! Which tasks the user pinned to the top of their project's list.

use rusqlite::{params, Connection};

use crate::StoreError;

/// Pins or unpins a task. Pinning twice keeps the first pin time.
///
/// # Errors
/// Fails for an unknown task, or when the database call fails.
pub fn set_task_pinned(
    connection: &Connection,
    task_id: &str,
    pinned: bool,
    at_ms: i64,
) -> Result<(), StoreError> {
    if pinned {
        connection.execute(
            "INSERT OR IGNORE INTO task_pins (task_id, pinned_at_ms) VALUES (?1, ?2)",
            params![task_id, at_ms],
        )?;
    } else {
        connection.execute("DELETE FROM task_pins WHERE task_id = ?1", [task_id])?;
    }
    Ok(())
}

/// The ids of the pinned tasks, earliest pin first.
///
/// # Errors
/// Fails when the query fails.
pub fn pinned_task_ids(connection: &Connection) -> Result<Vec<String>, StoreError> {
    let mut statement =
        connection.prepare("SELECT task_id FROM task_pins ORDER BY pinned_at_ms, task_id")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{open_in_memory, NewRepo, NewTask, RepoRecord, TaskRecord};
    use std::path::Path;

    fn connection_with_task(id: &str) -> Connection {
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
        TaskRecord::insert(
            &connection,
            &NewTask {
                id,
                repo_id: "r",
                parent_task_id: None,
                title: "t",
                prompt: "p",
                branch: "b",
                worktree_path: Path::new("/w"),
                agent_kind: None,
                agent_session_id: None,
                compare_group: None,
                created_at_ms: 1,
            },
        )
        .expect("task");
        connection
    }

    #[test]
    fn a_pin_is_kept_once_and_removed_on_unpin() {
        let connection = connection_with_task("a");
        set_task_pinned(&connection, "a", true, 5).expect("pin");
        set_task_pinned(&connection, "a", true, 9).expect("pin again");
        assert_eq!(pinned_task_ids(&connection).expect("list"), ["a"]);
        let at: i64 = connection
            .query_row("SELECT pinned_at_ms FROM task_pins", [], |row| row.get(0))
            .expect("time");
        assert_eq!(at, 5);
        set_task_pinned(&connection, "a", false, 10).expect("unpin");
        assert!(pinned_task_ids(&connection).expect("list").is_empty());
    }

    #[test]
    fn deleting_a_task_drops_its_pin() {
        let connection = connection_with_task("a");
        set_task_pinned(&connection, "a", true, 5).expect("pin");
        TaskRecord::delete(&connection, "a").expect("delete");
        assert!(pinned_task_ids(&connection).expect("list").is_empty());
    }

    #[test]
    fn an_unknown_task_cannot_be_pinned() {
        let connection = connection_with_task("a");
        assert!(set_task_pinned(&connection, "missing", true, 5).is_err());
    }
}
