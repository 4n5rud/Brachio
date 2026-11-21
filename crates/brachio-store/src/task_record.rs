//! Tasks: one worktree and branch each, with the agent that works in it.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::database::StoreError;
use crate::task_activity_record::record_activity;

/// A task as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    /// Stable id.
    pub id: String,
    /// Repository the task belongs to.
    pub repo_id: String,
    /// Optional parent task in the same repository.
    pub parent_task_id: Option<String>,
    /// Title the user gave.
    pub title: String,
    /// User's original task prompt.
    pub prompt: String,
    /// Branch created for the task.
    pub branch: String,
    /// Worktree folder.
    pub worktree_path: PathBuf,
    /// Agent the task was created for, as its snake_case name.
    pub agent_kind: Option<String>,
    /// Conversation id the agent resumes, when Brachio picked one.
    pub agent_session_id: Option<String>,
    /// When the task was created.
    pub created_at_ms: i64,
    /// Set once the task is archived.
    pub archived_at_ms: Option<i64>,
}

/// Fields for a new task row.
#[derive(Debug, Clone)]
pub struct NewTask<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Repository the task belongs to.
    pub repo_id: &'a str,
    /// Optional parent task.
    pub parent_task_id: Option<&'a str>,
    /// Title the user gave.
    pub title: &'a str,
    /// User's original task prompt.
    pub prompt: &'a str,
    /// Branch created for the task.
    pub branch: &'a str,
    /// Worktree folder.
    pub worktree_path: &'a Path,
    /// Agent the task is for, if any.
    pub agent_kind: Option<&'a str>,
    /// Conversation id to start the agent with, when the agent accepts one.
    pub agent_session_id: Option<&'a str>,
    /// Creation time.
    pub created_at_ms: i64,
}

/// Why [`TaskRecord::insert_child`] did not add the task.
#[derive(Debug, thiserror::Error)]
pub enum ChildInsertError {
    /// The parent already has the most open children allowed.
    #[error("the parent already has the most open child tasks")]
    LimitReached,
    /// The parent does not exist or was archived.
    #[error("the parent task is no longer open")]
    ParentClosed,
    /// The parent is itself a child, and children cannot have children.
    #[error("the parent task is itself a child task")]
    ParentIsChild,
    /// The database failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

const COLUMNS: &str =
    "id, repo_id, title, branch, worktree_path, agent_kind, created_at_ms, archived_at_ms, \
     agent_session_id, prompt, parent_task_id";

impl TaskRecord {
    /// Adds a task.
    ///
    /// # Errors
    /// Fails when the repository does not exist or the id is taken.
    pub fn insert(connection: &Connection, task: &NewTask<'_>) -> Result<Self, StoreError> {
        connection.execute(
            "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path,
                                agent_kind, state, created_at_ms, agent_session_id, parent_task_id)
             VALUES (?1, ?2, ?3, ?9, ?4, ?5, ?6, 'active', ?7, ?8, ?10)",
            params![
                task.id,
                task.repo_id,
                task.title,
                task.branch,
                task.worktree_path.to_string_lossy(),
                task.agent_kind,
                task.created_at_ms,
                task.agent_session_id,
                task.prompt,
                task.parent_task_id,
            ],
        )?;
        Self::get(connection, task.id)
    }

    /// Adds a child of `task.parent_task_id`, checking in the same transaction that the parent
    /// is open, is not a child, and has fewer than `max_open_children` open children.
    ///
    /// # Errors
    /// Fails with the reason the child was refused, or when the database fails.
    pub fn insert_child(
        connection: &mut Connection,
        task: &NewTask<'_>,
        max_open_children: i64,
    ) -> Result<Self, ChildInsertError> {
        Self::insert_child_after(connection, task, max_open_children, &[])
    }

    /// Like [`Self::insert_child`], and records in the same transaction that the child waits
    /// for each of the sibling tasks `after`, so nothing can start it before its dependencies
    /// exist.
    ///
    /// # Errors
    /// Fails with the reason the child was refused, or when the database fails, for instance
    /// for a task that does not exist.
    pub fn insert_child_after(
        connection: &mut Connection,
        task: &NewTask<'_>,
        max_open_children: i64,
        after: &[String],
    ) -> Result<Self, ChildInsertError> {
        let parent_id = task.parent_task_id.ok_or(ChildInsertError::ParentClosed)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(StoreError::from)?;
        let parent: Option<(Option<String>, Option<i64>)> = transaction
            .query_row(
                "SELECT parent_task_id, archived_at_ms FROM tasks WHERE id = ?1",
                [parent_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(StoreError::from)?;
        match parent {
            None | Some((_, Some(_))) => return Err(ChildInsertError::ParentClosed),
            Some((Some(_), None)) => return Err(ChildInsertError::ParentIsChild),
            Some((None, None)) => {}
        }
        let open: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE parent_task_id = ?1 AND archived_at_ms IS NULL",
                [parent_id],
                |row| row.get(0),
            )
            .map_err(StoreError::from)?;
        if open >= max_open_children {
            return Err(ChildInsertError::LimitReached);
        }
        let record = Self::insert(&transaction, task)?;
        record_activity(
            &transaction,
            parent_id,
            "child_created",
            Some(task.title),
            task.created_at_ms,
        )?;
        for predecessor in after {
            transaction
                .execute(
                    "INSERT INTO task_dependencies (task_id, depends_on) VALUES (?1, ?2)",
                    [task.id, predecessor.as_str()],
                )
                .map_err(StoreError::from)?;
        }
        transaction.commit().map_err(StoreError::from)?;
        Ok(record)
    }

    /// Tasks that are not archived, newest first.
    ///
    /// # Errors
    /// Fails when the query fails.
    pub fn list_open(connection: &Connection) -> Result<Vec<Self>, StoreError> {
        let sql = format!(
            "SELECT {COLUMNS} FROM tasks WHERE archived_at_ms IS NULL ORDER BY created_at_ms DESC"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], Self::from_row)?;
        let tasks: Vec<Self> = rows.collect::<Result<_, _>>()?;
        Ok(tasks)
    }

    /// Number of tasks, archived or not, in a repository.
    ///
    /// # Errors
    /// Fails when the query fails.
    pub fn count_for_repo(connection: &Connection, repo_id: &str) -> Result<u64, StoreError> {
        let count: i64 = connection.query_row(
            "SELECT count(*) FROM tasks WHERE repo_id = ?1",
            [repo_id],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// One task by id.
    ///
    /// # Errors
    /// Fails with `NotFound` when no task has this id.
    pub fn get(connection: &Connection, id: &str) -> Result<Self, StoreError> {
        let sql = format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1");
        let task = connection
            .query_row(&sql, [id], Self::from_row)
            .optional()?;
        task.ok_or_else(|| StoreError::NotFound {
            table: "tasks",
            id: id.to_owned(),
        })
    }

    /// Hides a task from the open list but keeps its history.
    ///
    /// # Errors
    /// Fails with `NotFound` when no task has this id.
    pub fn archive(connection: &Connection, id: &str, now_ms: i64) -> Result<(), StoreError> {
        Self::set_archived_at(connection, id, Some(now_ms))
    }

    /// Lists archived tasks, most recently archived first.
    ///
    /// # Errors
    /// Fails if the database cannot read the records.
    pub fn list_archived(connection: &Connection) -> Result<Vec<Self>, StoreError> {
        let sql = format!(
            "SELECT {COLUMNS} FROM tasks WHERE archived_at_ms IS NOT NULL ORDER BY archived_at_ms DESC"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], Self::from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<Self>>>()?)
    }

    /// Returns an archived task to the open list and invalidates its previous worker report.
    ///
    /// # Errors
    /// Fails when the record cannot be updated or does not exist.
    pub fn restore(connection: &Connection, id: &str) -> Result<Self, StoreError> {
        Self::set_archived_at(connection, id, None)?;
        Self::get(connection, id)
    }

    fn set_archived_at(
        connection: &Connection,
        id: &str,
        archived_at_ms: Option<i64>,
    ) -> Result<(), StoreError> {
        let changed = connection.execute(
            "UPDATE tasks
                SET archived_at_ms = ?2,
                    worker_report_generation = worker_report_generation
                        + (archived_at_ms IS NOT NULL AND ?2 IS NULL)
              WHERE id = ?1",
            params![id, archived_at_ms],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound {
                table: "tasks",
                id: id.to_owned(),
            });
        }
        Ok(())
    }

    /// Deletes a task row for good.
    ///
    /// # Errors
    /// Fails when sessions still point at the task.
    pub fn delete(connection: &Connection, id: &str) -> Result<(), StoreError> {
        connection.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Returns whether another task still references this task as a parent.
    ///
    /// # Errors
    /// Fails if SQLite cannot query the task records.
    pub fn has_children(connection: &Connection, id: &str) -> Result<bool, StoreError> {
        Ok(connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE parent_task_id = ?1)",
            [id],
            |row| row.get(0),
        )?)
    }

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let worktree_path: String = row.get(4)?;
        Ok(Self {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            title: row.get(2)?,
            branch: row.get(3)?,
            worktree_path: PathBuf::from(worktree_path),
            agent_kind: row.get(5)?,
            created_at_ms: row.get(6)?,
            archived_at_ms: row.get(7)?,
            agent_session_id: row.get(8)?,
            prompt: row.get(9)?,
            parent_task_id: row.get(10)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;
    use crate::repo_record::{NewRepo, RepoRecord};

    fn connection_with_repo() -> Connection {
        let connection = database::open_in_memory().expect("open database");
        let repo = NewRepo {
            id: "r1",
            name: "brachio",
            path: Path::new("/src/brachio"),
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(&connection, &repo).expect("insert repo");
        connection
    }

    fn sample<'a>(id: &'a str, created_at_ms: i64) -> NewTask<'a> {
        NewTask {
            id,
            repo_id: "r1",
            parent_task_id: None,
            title: "Fix login",
            prompt: "Fix login without changing the API",
            branch: "brachio/fix-login-a1b2c3",
            worktree_path: Path::new("/src/brachio.worktrees/fix-login-a1b2c3"),
            agent_kind: Some("claude_code"),
            agent_session_id: None,
            created_at_ms,
        }
    }

    #[test]
    fn open_tasks_are_listed_newest_first() {
        let connection = connection_with_repo();
        let first = sample("t1", 1);
        TaskRecord::insert(&connection, &first).expect("insert t1");
        assert_eq!(
            TaskRecord::get(&connection, first.id)
                .expect("read t1")
                .prompt,
            first.prompt
        );
        TaskRecord::insert(&connection, &sample("t2", 2)).expect("insert t2");

        let ids: Vec<String> = TaskRecord::list_open(&connection)
            .expect("list")
            .into_iter()
            .map(|task| task.id)
            .collect();

        assert_eq!(ids, vec!["t2", "t1"]);
    }

    #[test]
    fn archived_tasks_leave_the_open_list() {
        let connection = connection_with_repo();
        TaskRecord::insert(&connection, &sample("t1", 1)).expect("insert");

        TaskRecord::archive(&connection, "t1", 5).expect("archive");

        assert!(
            TaskRecord::list_open(&connection).expect("list").is_empty(),
            "t1 is archived"
        );
        assert_eq!(
            TaskRecord::get(&connection, "t1")
                .expect("get")
                .archived_at_ms,
            Some(5)
        );
        assert_eq!(
            TaskRecord::count_for_repo(&connection, "r1").expect("count"),
            1
        );
        assert_eq!(
            TaskRecord::list_archived(&connection)
                .expect("list archived")
                .len(),
            1
        );
        let restored = TaskRecord::restore(&connection, "t1").expect("restore");
        assert_eq!(restored.archived_at_ms, None);
        assert!(TaskRecord::list_archived(&connection)
            .expect("list archived")
            .is_empty());
        assert_eq!(
            TaskRecord::list_open(&connection).expect("list open").len(),
            1
        );
    }

    fn child<'a>(id: &'a str, parent: &'a str) -> NewTask<'a> {
        NewTask {
            parent_task_id: Some(parent),
            ..sample(id, 1)
        }
    }

    fn lead_with_children(count: usize) -> Connection {
        let connection = connection_with_repo();
        TaskRecord::insert(&connection, &sample("lead", 0)).expect("insert lead");
        for index in 0..count {
            let id = format!("c{index}");
            TaskRecord::insert(&connection, &child(&id, "lead")).expect("insert child");
        }
        connection
    }

    #[test]
    fn only_one_of_two_children_fits_under_the_limit() {
        let mut connection = lead_with_children(3);
        let first = TaskRecord::insert_child(&mut connection, &child("n1", "lead"), 4);
        let second = TaskRecord::insert_child(&mut connection, &child("n2", "lead"), 4);
        assert!(first.is_ok());
        assert!(matches!(second, Err(ChildInsertError::LimitReached)));
        assert!(TaskRecord::get(&connection, "n2").is_err());
    }

    #[test]
    fn archived_children_do_not_count_toward_the_limit() {
        let mut connection = lead_with_children(4);
        TaskRecord::archive(&connection, "c0", 5).expect("archive");
        assert!(TaskRecord::insert_child(&mut connection, &child("n1", "lead"), 4).is_ok());
    }

    #[test]
    fn a_child_cannot_be_a_parent() {
        let mut connection = lead_with_children(1);
        let refused = TaskRecord::insert_child(&mut connection, &child("n1", "c0"), 4);
        assert!(matches!(refused, Err(ChildInsertError::ParentIsChild)));
    }

    #[test]
    fn an_archived_or_missing_parent_is_refused() {
        let mut connection = lead_with_children(0);
        TaskRecord::archive(&connection, "lead", 5).expect("archive");
        let archived = TaskRecord::insert_child(&mut connection, &child("n1", "lead"), 4);
        assert!(matches!(archived, Err(ChildInsertError::ParentClosed)));
        let missing = TaskRecord::insert_child(&mut connection, &child("n2", "ghost"), 4);
        assert!(matches!(missing, Err(ChildInsertError::ParentClosed)));
    }

    #[test]
    fn a_task_needs_an_existing_repo() {
        let connection = database::open_in_memory().expect("open database");
        let orphan = TaskRecord::insert(&connection, &sample("t1", 1));
        assert!(orphan.is_err(), "foreign keys are on");
    }
}
