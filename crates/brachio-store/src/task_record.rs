//! Tasks: one worktree and branch each, with the agent that works in it.

use std::path::{Path, PathBuf};

use brachio_core::task_graph::TaskState;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::database::StoreError;
use crate::orchestration_record::load_graph;
use crate::orchestration_schedule::scheduler_enabled;
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
    /// Shared by the sibling tasks that run one prompt with different agents, to compare them.
    pub compare_group: Option<String>,
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
    /// Comparison group the task belongs to, if any.
    pub compare_group: Option<&'a str>,
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
    /// The parent's scheduler is off, so a task could not wait for another.
    #[error(
        "\"Start automatically\" is off for the parent task, so the child cannot wait for others"
    )]
    SchedulerOff,
    /// A task to wait for is not an open sibling, or can no longer finish.
    #[error(
        "{0} is no longer an open task that can finish, so a task waiting for it would never run"
    )]
    PredecessorUnavailable(String),
    /// The database failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Longest title a task can be renamed to.
pub const MAX_TITLE_CHARS: usize = 200;

const COLUMNS: &str =
    "id, repo_id, title, branch, worktree_path, agent_kind, created_at_ms, archived_at_ms, \
     agent_session_id, prompt, parent_task_id, compare_group";

impl TaskRecord {
    /// Adds a task.
    ///
    /// # Errors
    /// Fails when the repository does not exist or the id is taken.
    pub fn insert(connection: &Connection, task: &NewTask<'_>) -> Result<Self, StoreError> {
        connection.execute(
            "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path,
                                agent_kind, state, created_at_ms, agent_session_id, parent_task_id,
                                compare_group)
             VALUES (?1, ?2, ?3, ?9, ?4, ?5, ?6, 'active', ?7, ?8, ?10, ?11)",
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
                task.compare_group,
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
        check_predecessors(&transaction, parent_id, after)?;
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
            "SELECT {COLUMNS} FROM tasks WHERE archived_at_ms IS NULL ORDER BY sort_order IS NOT NULL, sort_order, created_at_ms DESC"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], Self::from_row)?;
        let tasks: Vec<Self> = rows.collect::<Result<_, _>>()?;
        Ok(tasks)
    }

    /// Gives a task a new title.
    ///
    /// # Errors
    /// Fails with `Invalid` for a blank or over-long title, and `NotFound` for an unknown task.
    pub fn rename(connection: &Connection, id: &str, title: &str) -> Result<(), StoreError> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
            return Err(StoreError::Invalid(format!(
                "A task title needs 1 to {MAX_TITLE_CHARS} characters."
            )));
        }
        let changed = connection.execute(
            "UPDATE tasks SET title = ?2 WHERE id = ?1",
            params![id, title],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound {
                table: "tasks",
                id: id.to_owned(),
            });
        }
        Ok(())
    }

    /// Places the open tasks of one project in the given order.
    ///
    /// # Errors
    /// Fails when an id is unknown or belongs to another project than the first one, and changes
    /// nothing.
    pub fn reorder(connection: &mut Connection, ids: &[String]) -> Result<(), StoreError> {
        let transaction = connection.transaction()?;
        let mut project: Option<String> = None;
        for (position, id) in ids.iter().enumerate() {
            let repo_id: Option<String> = transaction
                .query_row("SELECT repo_id FROM tasks WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()?
                .ok_or_else(|| StoreError::NotFound {
                    table: "tasks",
                    id: id.clone(),
                })?;
            let repo_id = repo_id.unwrap_or_default();
            if *project.get_or_insert_with(|| repo_id.clone()) != repo_id {
                return Err(StoreError::Invalid(
                    "Tasks can only be reordered inside their own project.".to_owned(),
                ));
            }
            transaction.execute(
                "UPDATE tasks SET sort_order = ?2 WHERE id = ?1",
                params![id, i64::try_from(position).unwrap_or(i64::MAX)],
            )?;
        }
        transaction.commit()?;
        Ok(())
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

    /// Deletes every task of a repository, archived or not. A task elsewhere whose parent was one
    /// of them loses that parent rather than blocking the delete.
    ///
    /// # Errors
    /// Fails when the update or the delete fails.
    pub fn delete_for_repo(connection: &Connection, repo_id: &str) -> Result<usize, StoreError> {
        connection.execute(
            "UPDATE tasks SET parent_task_id = NULL
             WHERE parent_task_id IN (SELECT id FROM tasks WHERE repo_id = ?1)",
            [repo_id],
        )?;
        Ok(connection.execute("DELETE FROM tasks WHERE repo_id = ?1", [repo_id])?)
    }

    /// Returns whether a task in another repository is still queued or blocked behind a task of
    /// this one. Removing the repository would drop that edge and let the task start unapproved.
    ///
    /// # Errors
    /// Fails if SQLite cannot query the dependency records.
    pub fn repo_has_outside_waiters(
        connection: &Connection,
        repo_id: &str,
    ) -> Result<bool, StoreError> {
        Ok(connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM task_dependencies d
                 JOIN tasks target ON target.id = d.depends_on
                 JOIN tasks waiting ON waiting.id = d.task_id
                 LEFT JOIN task_orchestration o ON o.task_id = d.task_id
                 WHERE target.repo_id = ?1 AND waiting.repo_id <> ?1
                   AND COALESCE(o.state, 'queued') IN ('queued', 'blocked'))",
            [repo_id],
            |row| row.get(0),
        )?)
    }

    /// Ids of the tasks, archived or not, in a repository.
    ///
    /// # Errors
    /// Fails when the query fails.
    pub fn ids_for_repo(connection: &Connection, repo_id: &str) -> Result<Vec<String>, StoreError> {
        let mut statement = connection.prepare("SELECT id FROM tasks WHERE repo_id = ?1")?;
        let ids = statement.query_map([repo_id], |row| row.get(0))?;
        Ok(ids.collect::<rusqlite::Result<Vec<String>>>()?)
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

    /// Returns whether another task is still queued or blocked behind this one.
    ///
    /// Deleting the task would drop the dependency edge and let those tasks start unapproved.
    ///
    /// # Errors
    /// Fails if SQLite cannot query the dependency records.
    pub fn has_waiting_dependents(connection: &Connection, id: &str) -> Result<bool, StoreError> {
        Ok(connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM task_dependencies d
                 LEFT JOIN task_orchestration o ON o.task_id = d.task_id
                 WHERE d.depends_on = ?1 AND COALESCE(o.state, 'queued') IN ('queued', 'blocked'))",
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
            compare_group: row.get(11)?,
        })
    }
}

/// Repeats, inside the insert transaction, the checks made when the child was requested:
/// the request may have waited minutes for the user's answer.
fn check_predecessors(
    connection: &Connection,
    parent_id: &str,
    after: &[String],
) -> Result<(), ChildInsertError> {
    if after.is_empty() {
        return Ok(());
    }
    if !scheduler_enabled(connection, parent_id).map_err(|e| StoreError::Invalid(e.to_string()))? {
        return Err(ChildInsertError::SchedulerOff);
    }
    let graph =
        load_graph(connection, parent_id).map_err(|e| StoreError::Invalid(e.to_string()))?;
    for id in after {
        let open: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND parent_task_id = ?2
                               AND archived_at_ms IS NULL)",
                [id.as_str(), parent_id],
                |row| row.get(0),
            )
            .map_err(StoreError::from)?;
        let finishable = open
            && graph.state(id).is_ok_and(|state| {
                !matches!(
                    state,
                    TaskState::Failed | TaskState::Cancelled | TaskState::Blocked
                )
            });
        if !finishable {
            return Err(ChildInsertError::PredecessorUnavailable(id.clone()));
        }
    }
    Ok(())
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
            compare_group: None,
            created_at_ms,
        }
    }

    fn add_second_repo(connection: &Connection) {
        let repo = NewRepo {
            id: "r2",
            name: "other",
            path: Path::new("/src/other"),
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(connection, &repo).expect("insert r2");
    }

    #[test]
    fn deleting_a_repos_tasks_takes_children_and_archived_tasks_too() {
        let connection = connection_with_repo();
        add_second_repo(&connection);
        TaskRecord::insert(&connection, &sample("lead", 1)).expect("insert lead");
        let child = NewTask {
            parent_task_id: Some("lead"),
            ..sample("child", 2)
        };
        TaskRecord::insert(&connection, &child).expect("insert child");
        TaskRecord::insert(&connection, &sample("old", 3)).expect("insert old");
        TaskRecord::archive(&connection, "old", 4).expect("archive old");
        let elsewhere = NewTask {
            repo_id: "r2",
            parent_task_id: Some("lead"),
            ..sample("elsewhere", 5)
        };
        TaskRecord::insert(&connection, &elsewhere).expect("insert elsewhere");

        let deleted = TaskRecord::delete_for_repo(&connection, "r1").expect("delete");

        assert_eq!(deleted, 3);
        assert_eq!(
            TaskRecord::count_for_repo(&connection, "r1").expect("count"),
            0
        );
        let kept = TaskRecord::get(&connection, "elsewhere").expect("other repo's task stays");
        assert_eq!(kept.parent_task_id, None);
        RepoRecord::delete(&connection, "r1").expect("the repo can go now");
    }

    #[test]
    fn a_task_in_another_repo_waiting_on_this_one_is_noticed() {
        let connection = connection_with_repo();
        add_second_repo(&connection);
        TaskRecord::insert(&connection, &sample("here", 1)).expect("insert here");
        let waiting = NewTask {
            repo_id: "r2",
            ..sample("waiting", 2)
        };
        TaskRecord::insert(&connection, &waiting).expect("insert waiting");
        assert!(!TaskRecord::repo_has_outside_waiters(&connection, "r1").expect("check"));

        connection
            .execute(
                "INSERT INTO task_dependencies (task_id, depends_on) VALUES ('waiting', 'here')",
                [],
            )
            .expect("add edge");

        assert!(TaskRecord::repo_has_outside_waiters(&connection, "r1").expect("check"));
        assert!(!TaskRecord::repo_has_outside_waiters(&connection, "r2").expect("check"));
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
    fn renaming_trims_and_refuses_blank_titles() {
        let connection = connection_with_repo();
        TaskRecord::insert(&connection, &sample("t1", 1)).expect("insert");
        TaskRecord::rename(&connection, "t1", "  new name ").expect("rename");
        assert_eq!(
            TaskRecord::get(&connection, "t1").expect("get").title,
            "new name"
        );
        assert!(TaskRecord::rename(&connection, "t1", "  ").is_err());
        assert!(TaskRecord::rename(&connection, "missing", "x").is_err());
    }

    #[test]
    fn a_manual_order_wins_and_new_tasks_still_come_first() {
        let mut connection = connection_with_repo();
        for (id, at) in [("t1", 1), ("t2", 2), ("t3", 3)] {
            TaskRecord::insert(&connection, &sample(id, at)).expect("insert");
        }
        let order = |connection: &Connection| -> Vec<String> {
            TaskRecord::list_open(connection)
                .expect("list")
                .into_iter()
                .map(|task| task.id)
                .collect()
        };
        TaskRecord::reorder(
            &mut connection,
            &["t1".to_owned(), "t3".to_owned(), "t2".to_owned()],
        )
        .expect("reorder");
        assert_eq!(order(&connection), ["t1", "t3", "t2"]);
        TaskRecord::insert(&connection, &sample("t4", 4)).expect("insert");
        assert_eq!(order(&connection), ["t4", "t1", "t3", "t2"]);
        assert!(TaskRecord::reorder(&mut connection, &["nope".to_owned()]).is_err());
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

    #[test]
    fn the_comparison_group_is_stored_and_read_back() {
        let connection = connection_with_repo();
        let grouped = NewTask {
            compare_group: Some("g1"),
            ..sample("a", 0)
        };
        let record = TaskRecord::insert(&connection, &grouped).expect("insert");
        assert_eq!(record.compare_group.as_deref(), Some("g1"));
        let plain = TaskRecord::insert(&connection, &sample("b", 1)).expect("insert");
        assert_eq!(plain.compare_group, None);
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

    fn waiting_child(
        connection: &mut Connection,
        id: &str,
        after: &str,
    ) -> Result<(), ChildInsertError> {
        TaskRecord::insert_child_after(connection, &child(id, "lead"), 4, &[after.to_owned()])
            .map(|_| ())
    }

    #[test]
    fn a_child_cannot_wait_once_the_scheduler_is_off() {
        let mut connection = lead_with_children(1);
        let refused = waiting_child(&mut connection, "n1", "c0");
        assert!(matches!(refused, Err(ChildInsertError::SchedulerOff)));
        assert!(TaskRecord::get(&connection, "n1").is_err());
    }

    #[test]
    fn a_child_cannot_wait_for_a_failed_or_archived_task() {
        let mut connection = lead_with_children(2);
        crate::orchestration_schedule::set_scheduler_enabled(&connection, "lead", true)
            .expect("enable");
        connection
            .execute(
                "INSERT INTO task_orchestration (task_id, state, updated_at_ms) VALUES ('c0', 'failed', 0)",
                [],
            )
            .expect("fail c0");
        TaskRecord::archive(&connection, "c1", 5).expect("archive");
        let failed = waiting_child(&mut connection, "n1", "c0");
        assert!(matches!(
            failed,
            Err(ChildInsertError::PredecessorUnavailable(_))
        ));
        let archived = waiting_child(&mut connection, "n2", "c1");
        assert!(matches!(
            archived,
            Err(ChildInsertError::PredecessorUnavailable(_))
        ));
        assert!(TaskRecord::get(&connection, "n1").is_err());
    }

    #[test]
    fn a_child_can_wait_for_an_open_task_while_the_scheduler_is_on() {
        let mut connection = lead_with_children(1);
        crate::orchestration_schedule::set_scheduler_enabled(&connection, "lead", true)
            .expect("enable");
        assert!(waiting_child(&mut connection, "n1", "c0").is_ok());
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
