//! Daily token aggregates read from the agents' session logs, and plan limit snapshots.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::StoreError;

/// Snapshots of one limit window are at least this far apart.
pub const SNAPSHOT_GAP_MS: i64 = 30 * 60 * 1000;

/// Tokens of one session on one day in one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRow {
    /// Provider id.
    pub provider: String,
    /// UTC date.
    pub day: String,
    /// Working directory.
    pub project: String,
    /// Conversation id.
    pub session: String,
    /// Non-cached input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Cache reads.
    pub cache_read: u64,
    /// Cache writes.
    pub cache_write: u64,
}

/// A reading of one plan limit window.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitSnapshot {
    /// Provider id.
    pub provider: String,
    /// `session` or `weekly`.
    pub window: String,
    /// When it was read.
    pub at_ms: i64,
    /// Share used, 0 to 100.
    pub used_percent: f64,
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// The saved scan state of a log, if it was scanned before.
///
/// # Errors
/// Fails when the query fails.
pub fn usage_file_state(connection: &Connection, path: &str) -> Result<Option<String>, StoreError> {
    let mut statement = connection.prepare("SELECT state FROM usage_files WHERE path = ?1")?;
    let mut rows = statement.query([path])?;
    Ok(match rows.next()? {
        Some(row) => Some(row.get(0)?),
        None => None,
    })
}

/// Adds the new buckets of one log and saves its scan state, together or not at all.
///
/// # Errors
/// Fails when a statement fails.
pub fn usage_apply(
    connection: &mut Connection,
    provider: &str,
    path: &str,
    state: &str,
    rows: &[UsageRow],
) -> Result<(), StoreError> {
    let transaction = connection.transaction()?;
    for row in rows {
        transaction.execute(
            "INSERT INTO usage_daily (provider, day, project, session, input, output, cache_read, cache_write)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (provider, day, project, session) DO UPDATE SET
               input = input + excluded.input, output = output + excluded.output,
               cache_read = cache_read + excluded.cache_read, cache_write = cache_write + excluded.cache_write",
            params![
                row.provider,
                row.day,
                row.project,
                row.session,
                to_i64(row.input),
                to_i64(row.output),
                to_i64(row.cache_read),
                to_i64(row.cache_write)
            ],
        )?;
    }
    transaction.execute(
        "INSERT INTO usage_files (path, provider, state) VALUES (?1, ?2, ?3)
         ON CONFLICT (path) DO UPDATE SET state = excluded.state",
        params![path, provider, state],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Forgets every aggregate and scan state, so the next scan reads all logs again. Limit snapshots stay.
///
/// # Errors
/// Fails when a statement fails.
pub fn usage_reset(connection: &mut Connection) -> Result<(), StoreError> {
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM usage_daily", [])?;
    transaction.execute("DELETE FROM usage_files", [])?;
    transaction.commit()?;
    Ok(())
}

/// The rows from `since_day` on, optionally of one provider.
///
/// # Errors
/// Fails when the query fails.
pub fn usage_rows(
    connection: &Connection,
    provider: Option<&str>,
    since_day: &str,
) -> Result<Vec<UsageRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT provider, day, project, session, input, output, cache_read, cache_write
         FROM usage_daily WHERE day >= ?1 AND (?2 IS NULL OR provider = ?2) ORDER BY day",
    )?;
    let rows = statement.query_map(params![since_day, provider], |row| {
        let count = |at: usize| row.get::<_, i64>(at).map(|n| u64::try_from(n).unwrap_or(0));
        Ok(UsageRow {
            provider: row.get(0)?,
            day: row.get(1)?,
            project: row.get(2)?,
            session: row.get(3)?,
            input: count(4)?,
            output: count(5)?,
            cache_read: count(6)?,
            cache_write: count(7)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Records a limit reading unless the same window was recorded less than [`SNAPSHOT_GAP_MS`] ago.
/// Returns whether it was stored.
///
/// # Errors
/// Fails when a statement fails.
pub fn record_limit_snapshot(
    connection: &Connection,
    provider: &str,
    window: &str,
    at_ms: i64,
    used_percent: f64,
) -> Result<bool, StoreError> {
    let latest: Option<i64> = connection.query_row(
        "SELECT MAX(at_ms) FROM usage_limit_snapshots WHERE provider = ?1 AND window = ?2",
        params![provider, window],
        |row| row.get(0),
    )?;
    if latest.is_some_and(|last| at_ms - last < SNAPSHOT_GAP_MS) {
        return Ok(false);
    }
    connection.execute(
        "INSERT OR IGNORE INTO usage_limit_snapshots (provider, window, at_ms, used_percent) VALUES (?1, ?2, ?3, ?4)",
        params![provider, window, at_ms, used_percent.clamp(0.0, 100.0)],
    )?;
    Ok(true)
}

/// Limit readings since `since_ms`, oldest first.
///
/// # Errors
/// Fails when the query fails.
pub fn limit_history(connection: &Connection, since_ms: i64) -> Result<Vec<LimitSnapshot>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT provider, window, at_ms, used_percent FROM usage_limit_snapshots WHERE at_ms >= ?1 ORDER BY at_ms",
    )?;
    let rows = statement.query_map([since_ms], |row| {
        Ok(LimitSnapshot { provider: row.get(0)?, window: row.get(1)?, at_ms: row.get(2)?, used_percent: row.get(3)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_in_memory;

    fn row(day: &str, session: &str, output: u64) -> UsageRow {
        UsageRow {
            provider: "claude_code".to_owned(),
            day: day.to_owned(),
            project: "/p".to_owned(),
            session: session.to_owned(),
            input: 1,
            output,
            cache_read: 0,
            cache_write: 0,
        }
    }

    #[test]
    fn applying_twice_adds_up_and_keeps_the_state() {
        let mut connection = open_in_memory().expect("open");
        usage_apply(&mut connection, "claude_code", "/a.jsonl", "s1", &[row("2026-11-01", "a", 5)]).expect("apply");
        usage_apply(&mut connection, "claude_code", "/a.jsonl", "s2", &[row("2026-11-01", "a", 3)]).expect("apply");
        let rows = usage_rows(&connection, None, "2026-11-01").expect("rows");
        assert_eq!((rows.len(), rows[0].output, rows[0].input), (1, 8, 2));
        assert_eq!(usage_file_state(&connection, "/a.jsonl").expect("state").as_deref(), Some("s2"));
        assert_eq!(usage_file_state(&connection, "/none").expect("state"), None);
    }

    #[test]
    fn rows_are_limited_by_day_and_provider_and_reset_clears_them() {
        let mut connection = open_in_memory().expect("open");
        usage_apply(&mut connection, "claude_code", "/a", "s", &[row("2026-10-01", "a", 1), row("2026-11-01", "a", 2)]).expect("apply");
        assert_eq!(usage_rows(&connection, None, "2026-11-01").expect("rows").len(), 1);
        assert!(usage_rows(&connection, Some("codex"), "2026-01-01").expect("rows").is_empty());
        usage_reset(&mut connection).expect("reset");
        assert!(usage_rows(&connection, None, "2026-01-01").expect("rows").is_empty());
    }

    #[test]
    fn limit_snapshots_keep_a_gap_and_survive_a_reset() {
        let mut connection = open_in_memory().expect("open");
        assert!(record_limit_snapshot(&connection, "codex", "session", 1_000, 10.0).expect("first"));
        assert!(!record_limit_snapshot(&connection, "codex", "session", 1_000 + 60_000, 20.0).expect("too soon"));
        assert!(record_limit_snapshot(&connection, "codex", "session", 1_000 + SNAPSHOT_GAP_MS, 30.0).expect("later"));
        usage_reset(&mut connection).expect("reset");
        assert_eq!(limit_history(&connection, 0).expect("history").len(), 2);
    }
}
