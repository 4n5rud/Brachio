//! Opening the database and running migrations.

use std::path::Path;

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_repo_name_task_agent.sql"),
    include_str!("../migrations/0003_task_agent_session.sql"),
    include_str!("../migrations/0004_task_secondary_tabs.sql"),
    include_str!("../migrations/0005_task_pr_status.sql"),
    include_str!("../migrations/0006_messages.sql"),
    include_str!("../migrations/0007_repo_is_git.sql"),
    include_str!("../migrations/0008_retired_secondary_tabs.sql"),
    include_str!("../migrations/0009_worker_report_generation.sql"),
    include_str!("../migrations/0010_task_orchestration.sql"),
    include_str!("../migrations/0011_qa_results.sql"),
    include_str!("../migrations/0012_handoff_review_note.sql"),
    include_str!("../migrations/0013_orchestration_scheduler.sql"),
    include_str!("../migrations/0014_task_activity.sql"),
    include_str!("../migrations/0015_task_pins.sql"),
    include_str!("../migrations/0016_task_compare_group.sql"),
    include_str!("../migrations/0017_task_tabs.sql"),
    include_str!("../migrations/0018_manual_order.sql"),
    include_str!("../migrations/0019_task_tab_name.sql"),
    include_str!("../migrations/0020_project_cues.sql"),
    include_str!("../migrations/0021_message_dismissed.sql"),
    include_str!("../migrations/0022_usage_analytics.sql"),
    include_str!("../migrations/0023_prompt_history.sql"),
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
    /// The request is not acceptable as given.
    #[error("{0}")]
    Invalid(String),
    /// A newer Brachio changed the database in a way this version does not understand.
    #[error("this database was written by a newer Brachio (schema {found}, this one knows {known}); update Brachio")]
    NewerSchema {
        /// Schema version stored in the file.
        found: usize,
        /// Newest schema version this build can handle.
        known: usize,
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
    if current > MIGRATIONS.len() {
        return Err(StoreError::NewerSchema {
            found: current,
            known: MIGRATIONS.len(),
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(
            None,
            "user_version",
            i64::try_from(index + 1).unwrap_or(i64::MAX),
        )?;
        transaction.commit()?;
    }
    Ok(())
}

fn user_version(connection: &Connection) -> Result<usize, StoreError> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(usize::try_from(version).unwrap_or(0))
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
    fn a_database_from_a_newer_brachio_is_refused() {
        let mut connection = open_in_memory().expect("open database");
        connection
            .pragma_update(
                None,
                "user_version",
                i64::try_from(MIGRATIONS.len() + 1).unwrap_or(i64::MAX),
            )
            .expect("pretend a newer schema");
        let error = migrate(&mut connection).expect_err("must not run on a newer schema");
        assert!(matches!(error, StoreError::NewerSchema { .. }), "{error}");
    }

    #[test]
    fn qa_results_arrive_on_a_database_at_the_previous_version() {
        let mut connection = Connection::open_in_memory().expect("open");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        let previous = MIGRATIONS.len() - 1;
        for sql in &MIGRATIONS[..previous] {
            connection.execute_batch(sql).expect("old migration");
        }
        connection
            .pragma_update(
                None,
                "user_version",
                i64::try_from(previous).unwrap_or(i64::MAX),
            )
            .expect("set old version");
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);",
            )
            .expect("insert an existing task");
        migrate(&mut connection).expect("upgrade");
        connection
            .execute(
                "INSERT INTO qa_results (task_id, command, cwd, outcome, exit_code, duration_ms, commit_sha,
                    verified_by_brachio, stdout, stderr, truncated, retry_allowed, created_at_ms)
                 VALUES ('t', 'c', '/w', 'passed', 0, 1, 'a', 1, '', '', 0, 1, 0)",
                [],
            )
            .expect("existing task accepts a QA result");
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

    #[test]
    fn an_existing_database_keeps_its_order_and_can_then_be_reordered() {
        let mut connection = Connection::open_in_memory().expect("open");
        for sql in &MIGRATIONS[..17] {
            connection.execute_batch(sql).expect("old migration");
        }
        connection
            .pragma_update(None, "user_version", 17)
            .expect("version");
        connection
            .execute_batch(
                "INSERT INTO repos (id, name, path, default_branch, added_at_ms) VALUES
                    ('r1', 'one', '/1', 'main', 1), ('r2', 'two', '/2', 'main', 2);
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t1', 'r1', 't', 'p', 'b1', '/w1', 'active', 1),
                        ('t2', 'r1', 't', 'p', 'b2', '/w2', 'active', 2);
                 INSERT INTO task_tabs (task_id, tab_id, kind) VALUES ('t1', 'a', 'shell'), ('t1', 'b', 'shell');",
            )
            .expect("old rows");

        migrate(&mut connection).expect("upgrade");

        let repos: Vec<String> = crate::RepoRecord::list(&connection)
            .expect("repos")
            .into_iter()
            .map(|repo| repo.id)
            .collect();
        assert_eq!(repos, ["r1", "r2"]);
        crate::RepoRecord::reorder(&mut connection, &["r2".to_owned(), "r1".to_owned()])
            .expect("reorder");
        let repos: Vec<String> = crate::RepoRecord::list(&connection)
            .expect("repos")
            .into_iter()
            .map(|repo| repo.id)
            .collect();
        assert_eq!(repos, ["r2", "r1"]);
        let tasks: Vec<String> = crate::TaskRecord::list_open(&connection)
            .expect("tasks")
            .into_iter()
            .map(|task| task.id)
            .collect();
        assert_eq!(tasks, ["t2", "t1"]);
        crate::reorder_task_tabs(&mut connection, "t1", &["b".to_owned(), "a".to_owned()])
            .expect("tabs");
        let tabs: Vec<String> = crate::task_tabs(&connection, "t1")
            .expect("tabs")
            .into_iter()
            .map(|tab| tab.tab_id)
            .collect();
        assert_eq!(tabs, ["b", "a"]);
    }
}
