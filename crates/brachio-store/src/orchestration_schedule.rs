//! Opt-in scheduling of a lead's child tasks: which ready children may start now, and what a
//! restart does to children that were running. Nothing here launches a process; callers start
//! the tasks this module returns, once.

use brachio_core::task_graph::TaskState;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::orchestration_record::{load_graph, transition_in, OrchestrationError};

/// Children of one lead that may run at the same time.
pub const MAX_RUNNING_CHILDREN: usize = 2;

/// Why a child that was running when the app closed is failed on the next start.
pub const LOST_AFTER_RESTART: &str = "lost after restart";

/// Turns the scheduler on or off for a lead. It is off until someone turns it on.
///
/// # Errors
/// Fails when the task is unknown or the database fails.
pub fn set_scheduler_enabled(
    connection: &Connection,
    lead_id: &str,
    enabled: bool,
) -> Result<(), OrchestrationError> {
    connection.execute(
        "INSERT INTO lead_orchestration (task_id, scheduler_enabled) VALUES (?1, ?2)
         ON CONFLICT(task_id) DO UPDATE SET scheduler_enabled = excluded.scheduler_enabled",
        params![lead_id, enabled],
    )?;
    Ok(())
}

/// Whether the lead's scheduler is on.
///
/// # Errors
/// Fails when the database fails.
pub fn scheduler_enabled(
    connection: &Connection,
    lead_id: &str,
) -> Result<bool, OrchestrationError> {
    let enabled: Option<bool> = connection
        .query_row(
            "SELECT scheduler_enabled FROM lead_orchestration WHERE task_id = ?1",
            [lead_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(enabled.unwrap_or(false))
}

/// Leads whose scheduler is on.
///
/// # Errors
/// Fails when the database fails.
pub fn scheduled_leads(connection: &Connection) -> Result<Vec<String>, OrchestrationError> {
    let mut statement = connection.prepare(
        "SELECT o.task_id FROM lead_orchestration o JOIN tasks t ON t.id = o.task_id
         WHERE o.scheduler_enabled = 1 AND t.archived_at_ms IS NULL ORDER BY t.created_at_ms, t.id",
    )?;
    let leads = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(leads)
}

/// The reason a task was last failed by Brachio, if it was.
///
/// # Errors
/// Fails when the database fails.
pub fn state_reason(
    connection: &Connection,
    task_id: &str,
) -> Result<Option<String>, OrchestrationError> {
    let reason: Option<Option<String>> = connection
        .query_row(
            "SELECT reason FROM task_orchestration WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(reason.flatten())
}

/// Starts what may start, all in one transaction: queued children whose dependencies are all
/// completed become ready, and ready children become running while fewer than
/// [`MAX_RUNNING_CHILDREN`] run. Returns the children that were just moved to running, in
/// creation order; each is returned by exactly one call, so the caller launches each once.
/// Does nothing when the lead's scheduler is off.
///
/// # Errors
/// Fails when the graph cannot be read or the database fails.
pub fn schedule_next(
    connection: &mut Connection,
    lead_id: &str,
    now_ms: i64,
) -> Result<Vec<String>, OrchestrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if !scheduler_enabled(&transaction, lead_id)? {
        return Ok(Vec::new());
    }
    let open: Vec<String> = {
        let mut statement = transaction.prepare(
            "SELECT id FROM tasks WHERE parent_task_id = ?1 AND archived_at_ms IS NULL
             ORDER BY created_at_ms, id",
        )?;
        let rows = statement.query_map([lead_id], |row| row.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let graph = load_graph(&transaction, lead_id)?;
    let promote: Vec<String> = graph
        .ready_tasks()
        .into_iter()
        .filter(|id| open.iter().any(|open_id| open_id == id))
        .map(str::to_owned)
        .collect();
    for id in &promote {
        transition_in(&transaction, id, TaskState::Ready, now_ms)?;
    }
    let graph = load_graph(&transaction, lead_id)?;
    let mut running = 0;
    for id in &open {
        if graph.state(id)? == TaskState::Running {
            running += 1;
        }
    }
    let mut started = Vec::new();
    for id in &open {
        if running >= MAX_RUNNING_CHILDREN {
            break;
        }
        if graph.state(id)? == TaskState::Ready {
            transition_in(&transaction, id, TaskState::Running, now_ms)?;
            started.push(id.clone());
            running += 1;
        }
    }
    transaction.commit()?;
    Ok(started)
}

/// Children recorded as running, across all leads.
///
/// # Errors
/// Fails when the database fails.
pub fn running_children(connection: &Connection) -> Result<Vec<String>, OrchestrationError> {
    let mut statement = connection.prepare(
        "SELECT task_id FROM task_orchestration WHERE state = 'running' ORDER BY task_id",
    )?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Fails each of these tasks that is still running, with [`LOST_AFTER_RESTART`], which also
/// blocks what depends on them. Returns the tasks that were failed. Nothing is relaunched.
///
/// # Errors
/// Fails when the database fails.
pub fn fail_lost(
    connection: &mut Connection,
    task_ids: &[String],
    now_ms: i64,
) -> Result<Vec<String>, OrchestrationError> {
    fail_running(connection, task_ids, LOST_AFTER_RESTART, now_ms)
}

/// Fails each of these tasks that is still running, recording `reason`, which also blocks what
/// depends on them. Returns the tasks that were failed.
///
/// # Errors
/// Fails when the database fails.
pub fn fail_running(
    connection: &mut Connection,
    task_ids: &[String],
    reason: &str,
    now_ms: i64,
) -> Result<Vec<String>, OrchestrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut failed = Vec::new();
    for id in task_ids {
        let state: Option<String> = transaction
            .query_row(
                "SELECT state FROM task_orchestration WHERE task_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        if state.as_deref() != Some("running") {
            continue;
        }
        transition_in(&transaction, id, TaskState::Failed, now_ms)?;
        transaction.execute(
            "UPDATE task_orchestration SET reason = ?2 WHERE task_id = ?1",
            params![id, reason],
        )?;
        failed.push(id.clone());
    }
    transaction.commit()?;
    Ok(failed)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{add_dependency, apply_transition, open_in_memory};

    fn db(children: &[&str]) -> Connection {
        let connection = open_in_memory().unwrap();
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('lead', 'r', 'L', 'p', 'b', '/w', 'new', 0);",
            )
            .unwrap();
        for (i, id) in children.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO tasks (id, repo_id, parent_task_id, title, prompt, branch,
                                        worktree_path, state, created_at_ms)
                     VALUES (?1, 'r', 'lead', ?1, 'p', ?1, '/w', 'new', ?2)",
                    params![id, i64::try_from(i + 1).unwrap()],
                )
                .unwrap();
        }
        connection
    }

    fn state(connection: &Connection, id: &str) -> TaskState {
        load_graph(connection, "lead").unwrap().state(id).unwrap()
    }

    #[test]
    fn nothing_starts_until_the_lead_opts_in() {
        let mut db = db(&["a"]);
        assert!(schedule_next(&mut db, "lead", 1).unwrap().is_empty());
        assert_eq!(state(&db, "a"), TaskState::Queued);
        set_scheduler_enabled(&db, "lead", true).unwrap();
        assert_eq!(schedule_next(&mut db, "lead", 2).unwrap(), vec!["a"]);
        assert!(schedule_next(&mut db, "lead", 3).unwrap().is_empty());
    }

    #[test]
    fn at_most_two_children_run_and_dependents_wait_for_completion() {
        let mut db = db(&["a", "b", "c", "d"]);
        add_dependency(&mut db, "d", "a").unwrap();
        set_scheduler_enabled(&db, "lead", true).unwrap();
        assert_eq!(schedule_next(&mut db, "lead", 1).unwrap(), vec!["a", "b"]);
        assert_eq!(state(&db, "c"), TaskState::Ready);
        assert_eq!(state(&db, "d"), TaskState::Queued);
        apply_transition(&mut db, "b", TaskState::ReviewRequired, 2).unwrap();
        assert_eq!(schedule_next(&mut db, "lead", 3).unwrap(), vec!["c"]);
        assert_eq!(state(&db, "d"), TaskState::Queued);
    }

    #[test]
    fn a_failed_predecessor_blocks_dependents_and_they_never_start() {
        let mut db = db(&["a", "b"]);
        add_dependency(&mut db, "b", "a").unwrap();
        set_scheduler_enabled(&db, "lead", true).unwrap();
        schedule_next(&mut db, "lead", 1).unwrap();
        apply_transition(&mut db, "a", TaskState::Failed, 2).unwrap();
        assert_eq!(state(&db, "b"), TaskState::Blocked);
        assert!(schedule_next(&mut db, "lead", 3).unwrap().is_empty());
    }

    #[test]
    fn an_unreviewed_predecessor_keeps_its_dependent_waiting() {
        let mut db = db(&["a", "b"]);
        add_dependency(&mut db, "b", "a").unwrap();
        set_scheduler_enabled(&db, "lead", true).unwrap();
        schedule_next(&mut db, "lead", 1).unwrap();
        apply_transition(&mut db, "a", TaskState::ReviewRequired, 2).unwrap();
        assert!(schedule_next(&mut db, "lead", 3).unwrap().is_empty());
        assert_eq!(state(&db, "b"), TaskState::Queued);
    }

    #[test]
    fn concurrent_scheduling_starts_each_child_once_and_never_exceeds_the_limit() {
        let ids: Vec<String> = (0..6).map(|i| format!("c{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let connection = db(&refs);
        set_scheduler_enabled(&connection, "lead", true).unwrap();
        let shared = Arc::new(Mutex::new(connection));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let shared = Arc::clone(&shared);
                std::thread::spawn(move || {
                    let mut started = Vec::new();
                    for _ in 0..5 {
                        let mut guard = shared.lock().unwrap();
                        started.extend(schedule_next(&mut guard, "lead", 1).unwrap());
                    }
                    started
                })
            })
            .collect();
        let mut all: Vec<String> = workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect();
        all.sort();
        assert_eq!(all, vec!["c0", "c1"]);
        let guard = shared.lock().unwrap();
        assert_eq!(
            running_children(&guard).unwrap().len(),
            MAX_RUNNING_CHILDREN
        );
    }

    #[test]
    fn a_running_child_that_is_gone_after_restart_fails_and_blocks_dependents() {
        let mut db = db(&["a", "b", "c"]);
        add_dependency(&mut db, "b", "a").unwrap();
        set_scheduler_enabled(&db, "lead", true).unwrap();
        schedule_next(&mut db, "lead", 1).unwrap();
        assert_eq!(running_children(&db).unwrap(), vec!["a", "c"]);
        let lost = vec!["a".to_owned()];
        assert_eq!(fail_lost(&mut db, &lost, 5).unwrap(), lost);
        assert_eq!(state(&db, "a"), TaskState::Failed);
        assert_eq!(state(&db, "b"), TaskState::Blocked);
        assert_eq!(state(&db, "c"), TaskState::Running);
        assert_eq!(
            state_reason(&db, "a").unwrap().as_deref(),
            Some(LOST_AFTER_RESTART)
        );
        assert!(fail_lost(&mut db, &lost, 6).unwrap().is_empty());
        assert!(schedule_next(&mut db, "lead", 7).unwrap().is_empty());
    }
}
