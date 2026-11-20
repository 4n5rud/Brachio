//! Durable secondary terminal identities. A NULL row records an explicit close.

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::StoreError;

/// Imports legacy tab IDs without overwriting a saved tab or close.
///
/// # Errors
/// Fails when the database cannot be read or written.
pub fn load_secondary_tabs(
    connection: &mut Connection,
    legacy: &HashMap<String, String>,
) -> Result<HashMap<String, String>, StoreError> {
    let transaction = connection.transaction()?;
    for (task_id, tab_id) in legacy {
        transaction.execute(
            "INSERT INTO task_secondary_tabs (task_id, tab_id)
             SELECT id, ?2 FROM tasks WHERE id = ?1
             ON CONFLICT(task_id) DO NOTHING",
            params![task_id, tab_id],
        )?;
    }
    let tabs = {
        let mut statement = transaction
            .prepare("SELECT task_id, tab_id FROM task_secondary_tabs WHERE tab_id IS NOT NULL")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    transaction.commit()?;
    Ok(tabs)
}

/// Saves a tab ID or remembers that the user closed it.
///
/// # Errors
/// Fails when the task is not an open task, or the database cannot be written.
pub fn set_secondary_tab(
    connection: &Connection,
    task_id: &str,
    tab_id: Option<&str>,
) -> Result<(), StoreError> {
    // The replaced tab's program may outlive this row, so its id is kept until proven stopped.
    connection.execute(
        "INSERT OR IGNORE INTO task_retired_secondary_tabs (task_id, tab_id)
         SELECT task_id, tab_id FROM task_secondary_tabs
         WHERE task_id = ?1 AND tab_id IS NOT NULL AND tab_id IS NOT ?2",
        params![task_id, tab_id],
    )?;
    let affected = connection.execute(
        "INSERT INTO task_secondary_tabs (task_id, tab_id)
         SELECT id, ?2 FROM tasks WHERE id = ?1 AND archived_at_ms IS NULL
         ON CONFLICT(task_id) DO UPDATE SET tab_id = excluded.tab_id",
        params![task_id, tab_id],
    )?;
    if affected == 0 {
        return Err(StoreError::NotFound {
            table: "open tasks",
            id: task_id.to_owned(),
        });
    }
    Ok(())
}

/// Tab IDs this task once had whose programs were not yet confirmed stopped.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn retired_secondary_tabs(
    connection: &Connection,
    task_id: &str,
) -> Result<Vec<String>, StoreError> {
    let mut statement =
        connection.prepare("SELECT tab_id FROM task_retired_secondary_tabs WHERE task_id = ?1")?;
    let rows = statement.query_map([task_id], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Drops a retired tab ID once its program is known to be stopped.
///
/// # Errors
/// Fails when the database cannot be written.
pub fn forget_retired_secondary_tab(
    connection: &Connection,
    task_id: &str,
    tab_id: &str,
) -> Result<(), StoreError> {
    connection.execute(
        "DELETE FROM task_retired_secondary_tabs WHERE task_id = ?1 AND tab_id = ?2",
        params![task_id, tab_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{forget_retired_secondary_tab, retired_secondary_tabs, set_secondary_tab};
    use std::path::Path;

    use crate::database::open_in_memory;
    use crate::{NewRepo, NewTask, RepoRecord, TaskRecord};

    fn connection_with_task() -> rusqlite::Connection {
        let connection = open_in_memory().expect("open");
        let repo = NewRepo {
            id: "r1",
            name: "brachio",
            path: Path::new("/src/brachio"),
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(&connection, &repo).expect("repo");
        let task = NewTask {
            id: "t1",
            repo_id: "r1",
            parent_task_id: None,
            title: "t",
            prompt: "p",
            branch: "b",
            worktree_path: Path::new("/w"),
            agent_kind: None,
            agent_session_id: None,
            created_at_ms: 0,
        };
        TaskRecord::insert(&connection, &task).expect("task");
        connection
    }

    #[test]
    fn a_replaced_or_closed_tab_is_remembered_until_forgotten() {
        let connection = connection_with_task();
        set_secondary_tab(&connection, "t1", Some("a")).expect("first");
        assert!(retired_secondary_tabs(&connection, "t1")
            .expect("list")
            .is_empty());
        set_secondary_tab(&connection, "t1", Some("b")).expect("replace");
        set_secondary_tab(&connection, "t1", None).expect("close");
        let mut retired = retired_secondary_tabs(&connection, "t1").expect("list");
        retired.sort();
        assert_eq!(retired, ["a", "b"]);
        forget_retired_secondary_tab(&connection, "t1", "a").expect("forget");
        assert_eq!(
            retired_secondary_tabs(&connection, "t1").expect("list"),
            ["b"]
        );
    }
}
