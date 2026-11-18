//! Opening the database and running migrations.

use std::path::Path;

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_repo_name_task_agent.sql"),
    include_str!("../migrations/0003_task_agent_session.sql"),
];

/// Why a storage call failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// SQLite returned an error.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// No row has the given id.
    #[error("no {table} row with id {id}")]
    NotFound {
        /// Table that was searched.
        table: &'static str,
        /// The id that was not found.
        id: String,
    },
}

/// Opens (or creates) the database and applies pending migrations.
///
/// # Errors
/// Fails when the file cannot be opened or a migration fails.
pub fn open(path: &Path) -> Result<Connection, StoreError> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    prepare(connection)
}

/// In-memory database for tests.
///
/// # Errors
/// Fails when a migration fails.
pub fn open_in_memory() -> Result<Connection, StoreError> {
    prepare(Connection::open_in_memory()?)
}

fn prepare(mut connection: Connection) -> Result<Connection, StoreError> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut connection)?;
    Ok(connection)
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let current = user_version(connection)?;
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", index + 1)?;
        transaction.commit()?;
    }
    Ok(())
}

fn user_version(connection: &Connection) -> Result<usize, StoreError> {
    let version: usize = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once() {
        let mut connection = open_in_memory().expect("open database");
        assert_eq!(
            user_version(&connection).expect("read version"),
            MIGRATIONS.len()
        );
        migrate(&mut connection).expect("second run is a no-op");
        let tables: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'tasks'",
                [],
                |row| row.get(0),
            )
            .expect("count tables");
        assert_eq!(tables, 1);
    }

    #[test]
    fn session_mode_is_constrained() {
        let connection = open_in_memory().expect("open database");
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);",
            )
            .expect("insert a repo and a task");
        let bad_mode = connection.execute(
            "INSERT INTO sessions (id, task_id, agent_kind, mode, status, started_at_ms)
             VALUES ('s', 't', 'codex', 'telnet', 'x', 0)",
            [],
        );
        assert!(
            bad_mode.is_err(),
            "the mode column must reject unknown modes"
        );
    }
}
