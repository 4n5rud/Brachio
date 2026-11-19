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
