//! Extra accounts for Claude Code and Codex, and the account each task was created with.
//!
//! An account is only a name and an id here. Its credentials live in a config folder that the
//! agent CLI owns; Brachio never reads or stores them. The login the CLI already has on this
//! computer is the built-in account and has no row.

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::StoreError;

/// Accounts one agent can have besides the built-in one.
pub const MAX_ACCOUNTS_PER_AGENT: usize = 20;
/// Longest account name.
pub const MAX_ACCOUNT_NAME_CHARS: usize = 40;

/// An extra account of one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAccount {
    /// Stable id; also the name of the account's config folder.
    pub id: String,
    /// Agent id, such as `claude_code`.
    pub agent: String,
    /// Name the user gave it.
    pub name: String,
    /// New tasks of this agent use it unless another account is chosen.
    pub is_default: bool,
    /// Creation time.
    pub created_at_ms: i64,
}

fn account_from(row: &Row<'_>) -> rusqlite::Result<AgentAccount> {
    Ok(AgentAccount {
        id: row.get(0)?,
        agent: row.get(1)?,
        name: row.get(2)?,
        is_default: row.get::<_, i64>(3)? != 0,
        created_at_ms: row.get(4)?,
    })
}

const COLUMNS: &str = "id, agent, name, is_default, created_at_ms";

fn clean_name(name: &str) -> Result<String, StoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StoreError::Invalid("Give the account a name.".to_owned()));
    }
    if name.chars().count() > MAX_ACCOUNT_NAME_CHARS {
        return Err(StoreError::Invalid(format!(
            "An account name has at most {MAX_ACCOUNT_NAME_CHARS} characters."
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(StoreError::Invalid(
            "An account name cannot contain control characters.".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

fn name_error(error: rusqlite::Error) -> StoreError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Invalid("An account with this name already exists.".to_owned())
        }
        _ => error.into(),
    }
}

/// Adds an account. `id` becomes the config folder's name, so it must be a plain token.
///
/// # Errors
/// Fails when the name is empty, too long or already used by this agent, when the agent has
/// too many accounts, or when the database fails.
pub fn add_agent_account(
    connection: &Connection,
    id: &str,
    agent: &str,
    name: &str,
    now_ms: i64,
) -> Result<AgentAccount, StoreError> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(StoreError::Invalid(
            "The account id is not valid.".to_owned(),
        ));
    }
    let name = clean_name(name)?;
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM agent_accounts WHERE agent = ?1",
        [agent],
        |row| row.get(0),
    )?;
    if usize::try_from(count).unwrap_or(usize::MAX) >= MAX_ACCOUNTS_PER_AGENT {
        return Err(StoreError::Invalid(format!(
            "An agent can have at most {MAX_ACCOUNTS_PER_AGENT} accounts."
        )));
    }
    connection
        .execute(
            "INSERT INTO agent_accounts (id, agent, name, is_default, created_at_ms)
             VALUES (?1, ?2, ?3, 0, ?4)",
            params![id, agent, name, now_ms],
        )
        .map_err(name_error)?;
    agent_account(connection, id)
}

/// One account by id.
///
/// # Errors
/// Fails when no account has the id or the database fails.
pub fn agent_account(connection: &Connection, id: &str) -> Result<AgentAccount, StoreError> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM agent_accounts WHERE id = ?1"),
            [id],
            account_from,
        )
        .optional()?
        .ok_or_else(|| StoreError::NotFound {
            table: "agent_accounts",
            id: id.to_owned(),
        })
}

/// The agent's extra accounts, oldest first.
///
/// # Errors
/// Fails when the database fails.
pub fn agent_accounts(
    connection: &Connection,
    agent: &str,
) -> Result<Vec<AgentAccount>, StoreError> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM agent_accounts WHERE agent = ?1 ORDER BY created_at_ms, id"
    ))?;
    let rows = statement.query_map([agent], account_from)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The agent's default extra account; `None` when the built-in account is the default.
///
/// # Errors
/// Fails when the database fails.
pub fn default_agent_account(
    connection: &Connection,
    agent: &str,
) -> Result<Option<AgentAccount>, StoreError> {
    Ok(connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM agent_accounts WHERE agent = ?1 AND is_default = 1"),
            [agent],
            account_from,
        )
        .optional()?)
}

/// Renames an account.
///
/// # Errors
/// Fails when the account does not exist, the name is not acceptable or is used by another
/// account of the same agent, or the database fails.
pub fn rename_agent_account(
    connection: &Connection,
    id: &str,
    name: &str,
) -> Result<(), StoreError> {
    let name = clean_name(name)?;
    let changed = connection
        .execute(
            "UPDATE agent_accounts SET name = ?2 WHERE id = ?1",
            params![id, name],
        )
        .map_err(name_error)?;
    if changed == 0 {
        return Err(StoreError::NotFound {
            table: "agent_accounts",
            id: id.to_owned(),
        });
    }
    Ok(())
}

/// Makes `id` the agent's default, or the built-in account the default when `id` is `None`.
///
/// # Errors
/// Fails when the account does not exist for this agent or the database fails.
pub fn set_default_agent_account(
    connection: &Connection,
    agent: &str,
    id: Option<&str>,
) -> Result<(), StoreError> {
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "UPDATE agent_accounts SET is_default = 0 WHERE agent = ?1",
        [agent],
    )?;
    if let Some(id) = id {
        let changed = transaction.execute(
            "UPDATE agent_accounts SET is_default = 1 WHERE id = ?1 AND agent = ?2",
            params![id, agent],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound {
                table: "agent_accounts",
                id: id.to_owned(),
            });
        }
    }
    Ok(transaction.commit()?)
}

/// Removes an account. Tasks that used it keep the id and stop launching.
///
/// # Errors
/// Fails when the account does not exist or the database fails.
pub fn remove_agent_account(connection: &Connection, id: &str) -> Result<(), StoreError> {
    let changed = connection.execute("DELETE FROM agent_accounts WHERE id = ?1", [id])?;
    if changed == 0 {
        return Err(StoreError::NotFound {
            table: "agent_accounts",
            id: id.to_owned(),
        });
    }
    Ok(())
}

/// Remembers the extra account a task was created with.
///
/// # Errors
/// Fails when the task does not exist or the database fails.
pub fn set_task_agent_account(
    connection: &Connection,
    task_id: &str,
    account_id: &str,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO task_agent_account (task_id, account_id) VALUES (?1, ?2)
         ON CONFLICT(task_id) DO UPDATE SET account_id = excluded.account_id",
        params![task_id, account_id],
    )?;
    Ok(())
}

/// The extra account of a task; `None` means the built-in account.
///
/// # Errors
/// Fails when the database fails.
pub fn task_agent_account(
    connection: &Connection,
    task_id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT account_id FROM task_agent_account WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )
        .optional()?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let connection = crate::open_in_memory().unwrap();
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);",
            )
            .unwrap();
        connection
    }

    #[test]
    fn an_account_is_added_listed_renamed_and_removed() {
        let db = db();
        add_agent_account(&db, "a1", "codex", " Work ", 1).unwrap();
        add_agent_account(&db, "a2", "codex", "Personal", 2).unwrap();
        add_agent_account(&db, "c1", "claude_code", "Work", 3).unwrap();
        let names: Vec<_> = agent_accounts(&db, "codex")
            .unwrap()
            .into_iter()
            .map(|a| a.name)
            .collect();
        assert_eq!(names, ["Work", "Personal"]);
        rename_agent_account(&db, "a2", "Side").unwrap();
        assert_eq!(agent_account(&db, "a2").unwrap().name, "Side");
        remove_agent_account(&db, "a1").unwrap();
        assert_eq!(agent_accounts(&db, "codex").unwrap().len(), 1);
        assert!(matches!(
            remove_agent_account(&db, "a1"),
            Err(StoreError::NotFound { .. })
        ));
    }

    #[test]
    fn names_must_be_filled_short_and_unique_per_agent() {
        let db = db();
        add_agent_account(&db, "a1", "codex", "Work", 1).unwrap();
        for bad in ["", "   ", &"x".repeat(MAX_ACCOUNT_NAME_CHARS + 1), "a\nb"] {
            assert!(
                matches!(
                    add_agent_account(&db, "a9", "codex", bad, 2),
                    Err(StoreError::Invalid(_))
                ),
                "{bad:?}"
            );
        }
        assert!(add_agent_account(&db, "a2", "codex", "work", 2).is_err());
        add_agent_account(&db, "c1", "claude_code", "Work", 2).unwrap();
        add_agent_account(&db, "a3", "codex", "Other", 3).unwrap();
        assert!(rename_agent_account(&db, "a3", "WORK").is_err());
        assert!(add_agent_account(&db, "../x", "codex", "Evil", 4).is_err());
    }

    #[test]
    fn an_agent_has_a_bounded_number_of_accounts() {
        let db = db();
        for n in 0..MAX_ACCOUNTS_PER_AGENT {
            add_agent_account(&db, &format!("a{n}"), "codex", &format!("n{n}"), 0).unwrap();
        }
        assert!(add_agent_account(&db, "zz", "codex", "one more", 0).is_err());
    }

    #[test]
    fn one_default_per_agent_and_none_means_the_built_in_account() {
        let db = db();
        assert!(default_agent_account(&db, "codex").unwrap().is_none());
        add_agent_account(&db, "a1", "codex", "A", 1).unwrap();
        add_agent_account(&db, "a2", "codex", "B", 2).unwrap();
        add_agent_account(&db, "c1", "claude_code", "C", 3).unwrap();
        set_default_agent_account(&db, "codex", Some("a1")).unwrap();
        set_default_agent_account(&db, "claude_code", Some("c1")).unwrap();
        set_default_agent_account(&db, "codex", Some("a2")).unwrap();
        assert_eq!(
            default_agent_account(&db, "codex").unwrap().unwrap().id,
            "a2"
        );
        assert!(!agent_account(&db, "a1").unwrap().is_default);
        assert_eq!(
            default_agent_account(&db, "claude_code")
                .unwrap()
                .unwrap()
                .id,
            "c1"
        );
        set_default_agent_account(&db, "codex", None).unwrap();
        assert!(default_agent_account(&db, "codex").unwrap().is_none());
        // An account of another agent cannot become the default, and the old default stays.
        set_default_agent_account(&db, "codex", Some("a1")).unwrap();
        assert!(set_default_agent_account(&db, "codex", Some("c1")).is_err());
        assert_eq!(
            default_agent_account(&db, "codex").unwrap().unwrap().id,
            "a1"
        );
    }

    #[test]
    fn a_task_remembers_its_account_even_after_the_account_is_removed() {
        let db = db();
        assert_eq!(task_agent_account(&db, "t").unwrap(), None);
        add_agent_account(&db, "a1", "codex", "A", 1).unwrap();
        set_task_agent_account(&db, "t", "a1").unwrap();
        remove_agent_account(&db, "a1").unwrap();
        assert_eq!(task_agent_account(&db, "t").unwrap().as_deref(), Some("a1"));
        db.execute("DELETE FROM tasks WHERE id = 't'", []).unwrap();
        assert_eq!(task_agent_account(&db, "t").unwrap(), None);
    }
}
