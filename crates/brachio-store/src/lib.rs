//! SQLite storage. One writer connection in WAL mode.

use std::path::Path;

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Opens (or creates) the database and applies pending migrations.
pub fn open(path: &Path) -> Result<Connection, StoreError> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    prepare(conn)
}

/// In-memory database for tests.
pub fn open_in_memory() -> Result<Connection, StoreError> {
    prepare(Connection::open_in_memory()?)
}

fn prepare(mut conn: Connection) -> Result<Connection, StoreError> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let current: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i + 1)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once() {
        let mut conn = open_in_memory().unwrap();
        let v: usize = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len());
        migrate(&mut conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'tasks'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn session_mode_is_constrained() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO repos VALUES ('r', '/x', 'main', NULL);
             INSERT INTO tasks VALUES ('t', 'r', NULL, 'T', 'p', 'b', '/w', NULL, 'new', 0);",
        )
        .unwrap();
        let bad = conn.execute(
            "INSERT INTO sessions VALUES ('s', 't', 'codex', 'telnet', NULL, NULL, 'x', NULL, 0)",
            [],
        );
        assert!(bad.is_err());
    }
}
