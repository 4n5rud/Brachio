//! Persistent, local development port assignment for task terminals.

use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;

use brachio_git::{PortBlock, PortPool};
use brachio_store::TaskRecord;
use rusqlite::{OptionalExtension, TransactionBehavior};
use tauri::State;

use crate::workspace::Database;

const FIRST_PORT: u16 = 41_000;
const PORT_COUNT: u16 = 1_000;

/// Returns or reserves the preferred local port for an open task.
#[tauri::command]
pub fn task_assign_port(database: State<'_, Database>, id: String) -> Result<u16, String> {
    assign(&database, &id)
}

/// Check that a key and working directory describe the same registered task.
pub fn port_for_terminal(
    database: &Database,
    key: &str,
    cwd: &Path,
) -> Result<Option<u16>, String> {
    let Some((id, pane)) = key.split_once(':') else {
        return Ok(None);
    };
    if !matches!(pane, "agent" | "shell")
        && pane
            .strip_prefix("agent-")
            .or_else(|| pane.strip_prefix("shell-"))
            .is_none_or(|id| uuid::Uuid::parse_str(id).is_err())
    {
        return Ok(None);
    }
    let task = TaskRecord::get(&*database.lock()?, id).map_err(|error| error.to_string())?;
    if task.archived_at_ms.is_some()
        || task
            .worktree_path
            .canonicalize()
            .map_err(|error| error.to_string())?
            != cwd.canonicalize().map_err(|error| error.to_string())?
    {
        return Ok(None);
    }
    assign(database, id).map(Some)
}

fn assign(database: &Database, id: &str) -> Result<u16, String> {
    let sql_error = |error: rusqlite::Error| error.to_string();
    let mut connection = database.lock()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let current = transaction
        .query_row(
            "SELECT port_base FROM tasks WHERE id = ?1 AND archived_at_ms IS NULL",
            [id],
            |row| row.get::<_, Option<u16>>(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| "Only open tasks can reserve a local port.".to_owned())?;
    if let Some(port) = current {
        // A restored task may have had its port given to another task while it was archived.
        let taken_by_another: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE port_base = ?1 AND id != ?2 AND archived_at_ms IS NULL",
                (port, id),
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if taken_by_another == 0 {
            return Ok(port);
        }
    }

    let mut pool = PortPool::new(FIRST_PORT, 1, PORT_COUNT);
    {
        let mut statement = transaction
            .prepare(
                "SELECT port_base FROM tasks WHERE port_base IS NOT NULL AND archived_at_ms IS NULL AND id != ?1",
            )
            .map_err(sql_error)?;
        let ports = statement
            .query_map([id], |row| row.get::<_, u16>(0))
            .map_err(sql_error)?;
        for port in ports {
            pool.restore(PortBlock {
                base: port.map_err(sql_error)?,
                len: 1,
            });
        }
    }
    let assigned = pool
        // Loopback is enough to see a busy port and does not raise a firewall prompt.
        .allocate(|block| TcpListener::bind((Ipv4Addr::LOCALHOST, block.base)).is_ok())
        .ok_or_else(|| "No free development port is available.".to_owned())?
        .base;
    transaction
        .execute(
            "UPDATE tasks SET port_base = ?2 WHERE id = ?1",
            (id, assigned),
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(assigned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Both tests use the first port of the pool; one holds it busy on purpose, so they must not
    /// run at the same time.
    static FIRST_PORT_TESTS: Mutex<()> = Mutex::new(());

    #[test]
    fn assignment_skips_busy_ports_and_remembers_task_reservations() {
        let _serial = FIRST_PORT_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let connection = brachio_store::open_in_memory().expect("open test database");
        connection.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', 'repo', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('first', 'r', 'First', '', 'first', 'first', 'active', 1),
                    ('second', 'r', 'Second', '', 'second', 'second', 'active', 2);"
        ).expect("insert test tasks");
        let database = Database(Mutex::new(connection));
        let busy = TcpListener::bind((Ipv4Addr::LOCALHOST, FIRST_PORT)).ok();
        let first = assign(&database, "first").expect("assign first");
        let second = assign(&database, "second").expect("assign second");
        if busy.is_some() {
            assert_ne!(first, FIRST_PORT);
            assert_ne!(second, FIRST_PORT);
        }
        assert_ne!(first, second);
        assert_eq!(
            assign(&database, "first").expect("read saved assignment"),
            first
        );
        assert_eq!(
            assign(&database, "second").expect("read saved assignment"),
            second
        );
    }

    #[test]
    fn an_archived_task_gives_its_port_back_and_a_restored_one_never_shares_it() {
        let _serial = FIRST_PORT_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let connection = brachio_store::open_in_memory().expect("open test database");
        connection.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', 'repo', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('first', 'r', 'First', '', 'first', 'first', 'active', 1),
                    ('third', 'r', 'Third', '', 'third', 'third', 'active', 3);"
        ).expect("insert test tasks");
        let database = Database(Mutex::new(connection));
        let first = assign(&database, "first").expect("assign first");

        database
            .lock()
            .expect("lock")
            .execute("UPDATE tasks SET archived_at_ms = 5 WHERE id = 'first'", [])
            .expect("archive");
        let third = assign(&database, "third").expect("assign third");
        assert_eq!(third, first, "the archived task's port is free again");

        database
            .lock()
            .expect("lock")
            .execute(
                "UPDATE tasks SET archived_at_ms = NULL WHERE id = 'first'",
                [],
            )
            .expect("restore");
        let again = assign(&database, "first").expect("assign restored task");
        assert_ne!(
            again, third,
            "a restored task must not share a port with an open one"
        );
    }
}
