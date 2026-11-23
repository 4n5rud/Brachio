//! Pull requests a user linked to a task besides the one of its own branch, for example a
//! follow-up or a stacked pull request.

use rusqlite::{params, Connection};

use crate::StoreError;

/// Most pull requests one task can link, so a mistake cannot grow the Inspector without end.
pub const MAX_LINKED_PRS: usize = 10;

/// One linked pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedPr {
    /// `owner/name`.
    pub repository: String,
    /// The pull request number in that repository.
    pub number: u64,
}

/// Links a pull request to a task. Linking the same one again changes nothing.
///
/// # Errors
/// Fails when the task does not exist, already has [`MAX_LINKED_PRS`] links, or the database fails.
pub fn add_linked_pr(
    connection: &Connection,
    task_id: &str,
    repository: &str,
    number: u64,
    now_ms: i64,
) -> Result<(), StoreError> {
    let number = i64::try_from(number).map_err(|_| StoreError::Invalid("bad number".to_owned()))?;
    let present: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_linked_prs WHERE task_id = ?1 AND repository = ?2 AND number = ?3)",
        params![task_id, repository, number],
        |row| row.get(0),
    )?;
    if present {
        return Ok(());
    }
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM task_linked_prs WHERE task_id = ?1",
        [task_id],
        |row| row.get(0),
    )?;
    if usize::try_from(count).unwrap_or(usize::MAX) >= MAX_LINKED_PRS {
        return Err(StoreError::Invalid(format!(
            "a task can link at most {MAX_LINKED_PRS} pull requests"
        )));
    }
    connection.execute(
        "INSERT INTO task_linked_prs (task_id, repository, number, added_at_ms) VALUES (?1, ?2, ?3, ?4)",
        params![task_id, repository, number, now_ms],
    )?;
    Ok(())
}

/// Removes a link. Removing one that is not there is fine.
///
/// # Errors
/// Fails when the database fails.
pub fn remove_linked_pr(
    connection: &Connection,
    task_id: &str,
    repository: &str,
    number: u64,
) -> Result<(), StoreError> {
    let number = i64::try_from(number).map_err(|_| StoreError::Invalid("bad number".to_owned()))?;
    connection.execute(
        "DELETE FROM task_linked_prs WHERE task_id = ?1 AND repository = ?2 AND number = ?3",
        params![task_id, repository, number],
    )?;
    Ok(())
}

/// The pull requests linked to a task, in the order they were linked.
///
/// # Errors
/// Fails when the database fails.
pub fn linked_prs(connection: &Connection, task_id: &str) -> Result<Vec<LinkedPr>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT repository, number FROM task_linked_prs WHERE task_id = ?1 ORDER BY added_at_ms, rowid",
    )?;
    let rows = statement.query_map([task_id], |row| {
        Ok(LinkedPr {
            repository: row.get(0)?,
            number: row.get::<_, i64>(1)?.try_into().unwrap_or(0),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
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
    fn links_keep_their_order_ignore_repeats_and_go_with_the_task() {
        let connection = with_task();
        add_linked_pr(&connection, "t", "o/r", 7, 1).expect("link");
        add_linked_pr(&connection, "t", "o/other", 3, 2).expect("link");
        add_linked_pr(&connection, "t", "o/r", 7, 3).expect("again");
        assert_eq!(
            linked_prs(&connection, "t").expect("list"),
            [
                LinkedPr {
                    repository: "o/r".to_owned(),
                    number: 7
                },
                LinkedPr {
                    repository: "o/other".to_owned(),
                    number: 3
                },
            ]
        );
        remove_linked_pr(&connection, "t", "o/r", 7).expect("remove");
        assert_eq!(linked_prs(&connection, "t").expect("list").len(), 1);
        connection
            .execute("DELETE FROM tasks WHERE id = 't'", [])
            .expect("delete");
        assert!(linked_prs(&connection, "t").expect("list").is_empty());
    }

    #[test]
    fn the_eleventh_link_is_refused() {
        let connection = with_task();
        for number in 1..=10 {
            add_linked_pr(&connection, "t", "o/r", number, 0).expect("link");
        }
        assert!(add_linked_pr(&connection, "t", "o/r", 11, 0).is_err());
    }
}
