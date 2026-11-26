//! The environment that makes an agent CLI use one of the extra accounts.
//!
//! Each extra account has its own config folder under the app's data folder. Claude Code reads
//! it through `CLAUDE_CONFIG_DIR` and Codex through `CODEX_HOME`, and the CLI keeps its sign-in
//! there. Brachio only passes the folder; it never reads or stores the credentials.

use std::path::{Path, PathBuf};

use brachio_agent::AgentKind;
use brachio_store::AgentAccount;
use rusqlite::Connection;

/// The folder that holds one config folder per extra account.
pub struct AccountsFolder(pub PathBuf);

/// What the new-task dialog sends to choose the login the CLI already has on this computer.
pub const BUILT_IN: &str = "builtin";

/// The variable through which the agent CLI is told where its config and sign-in live.
#[must_use]
pub const fn config_variable(kind: AgentKind) -> Option<&'static str> {
    match kind {
        AgentKind::ClaudeCode => Some("CLAUDE_CONFIG_DIR"),
        AgentKind::Codex => Some("CODEX_HOME"),
        AgentKind::Grok => None,
    }
}

/// The config folder of an account, created private to the user when it is first needed.
fn config_folder(root: &Path, account: &AgentAccount) -> Result<PathBuf, String> {
    let folder = root.join(&account.id);
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&folder).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(folder)
}

/// The environment variable that points `kind` at the account's folder.
///
/// # Errors
/// Fails when the account belongs to another agent, the agent has no account support, or the
/// folder cannot be created.
pub fn account_environment(
    root: &Path,
    kind: AgentKind,
    account: &AgentAccount,
) -> Result<Vec<(String, String)>, String> {
    let variable = config_variable(kind)
        .ok_or_else(|| format!("{} has no separate accounts.", kind.spec().name))?;
    if account.agent != kind.spec().id {
        return Err(format!(
            "The account {:?} is for another agent.",
            account.name
        ));
    }
    let folder = config_folder(root, account)?;
    // A machine-wide API key must not take precedence over the selected CLI login.
    let auth_variables: &[&str] = match kind {
        AgentKind::ClaudeCode => &[
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
        ],
        AgentKind::Codex => &[
            "OPENAI_API_KEY",
            "CODEX_API_KEY",
            "CODEX_ACCESS_TOKEN",
            "OPENAI_IDENTITY_TOKEN_FILE",
        ],
        AgentKind::Grok => &[],
    };
    let mut environment = vec![(variable.to_owned(), folder.to_string_lossy().into_owned())];
    environment.extend(
        auth_variables
            .iter()
            .map(|name| ((*name).to_owned(), String::new())),
    );
    Ok(environment)
}

/// The account a new task gets: the one chosen, else the agent's default. `None` is the
/// built-in account.
///
/// # Errors
/// Fails when the chosen account does not exist, is for another agent, or when `kind` has no
/// account support and one was chosen.
pub fn resolve_choice(
    connection: &Connection,
    kind: AgentKind,
    choice: Option<&str>,
) -> Result<Option<String>, String> {
    let agent = kind.spec().id;
    match choice {
        Some(BUILT_IN) => Ok(None),
        Some(id) => {
            let account = brachio_store::agent_account(connection, id)
                .map_err(|_| "That account no longer exists.".to_owned())?;
            if account.agent != agent {
                return Err(format!(
                    "The account {:?} is for another agent.",
                    account.name
                ));
            }
            Ok(Some(account.id))
        }
        None if config_variable(kind).is_none() => Ok(None),
        None => Ok(brachio_store::default_agent_account(connection, agent)
            .map_err(|error| error.to_string())?
            .map(|account| account.id)),
    }
}

/// What the task's account adds to a launch of `kind`: nothing for the built-in account. A task
/// whose account was removed stops here instead of starting under a different login.
///
/// # Errors
/// Fails when the task's account was removed or its folder cannot be created.
pub fn launch_environment(
    connection: &Connection,
    root: &Path,
    task_id: &str,
    kind: AgentKind,
) -> Result<Vec<(String, String)>, String> {
    let Some(id) = brachio_store::task_agent_account(connection, task_id)
        .map_err(|error| error.to_string())?
    else {
        return Ok(Vec::new());
    };
    let account = brachio_store::agent_account(connection, &id).map_err(|_| {
        "The account this task was created with has been removed. Create a new task with another account."
            .to_owned()
    })?;
    if account.agent != kind.spec().id {
        return Ok(Vec::new());
    }
    account_environment(root, kind, &account)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let db = brachio_store::open_in_memory().unwrap();
        db.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);",
        )
        .unwrap();
        db
    }

    #[test]
    fn each_agent_gets_its_own_variable_and_an_account_folder() {
        let root = tempfile::tempdir().unwrap();
        let db = db();
        let claude = brachio_store::add_agent_account(&db, "c1", "claude_code", "Work", 1).unwrap();
        let codex = brachio_store::add_agent_account(&db, "x1", "codex", "Work", 1).unwrap();
        let env = account_environment(root.path(), AgentKind::ClaudeCode, &claude).unwrap();
        assert_eq!(env[0].0, "CLAUDE_CONFIG_DIR");
        assert!(env.contains(&("CLAUDE_CODE_OAUTH_TOKEN".to_owned(), String::new())));
        assert_eq!(Path::new(&env[0].1), root.path().join("c1"));
        assert!(root.path().join("c1").is_dir());
        let env = account_environment(root.path(), AgentKind::Codex, &codex).unwrap();
        assert_eq!(env[0].0, "CODEX_HOME");
        assert!(env.contains(&("OPENAI_API_KEY".to_owned(), String::new())));
        assert!(account_environment(root.path(), AgentKind::Codex, &claude).is_err());
        assert!(account_environment(root.path(), AgentKind::Grok, &claude).is_err());
    }

    #[test]
    fn a_task_without_an_account_adds_nothing_and_a_removed_account_stops_the_launch() {
        let root = tempfile::tempdir().unwrap();
        let db = db();
        assert!(launch_environment(&db, root.path(), "t", AgentKind::Codex)
            .unwrap()
            .is_empty());
        brachio_store::add_agent_account(&db, "x1", "codex", "Work", 1).unwrap();
        brachio_store::set_task_agent_account(&db, "t", "x1").unwrap();
        let env = launch_environment(&db, root.path(), "t", AgentKind::Codex).unwrap();
        assert_eq!(env[0].0, "CODEX_HOME");
        // The task's other agent tabs are not changed by an account of Codex.
        assert!(
            launch_environment(&db, root.path(), "t", AgentKind::ClaudeCode)
                .unwrap()
                .is_empty()
        );
        brachio_store::remove_agent_account(&db, "x1").unwrap();
        let error = launch_environment(&db, root.path(), "t", AgentKind::Codex).unwrap_err();
        assert!(error.contains("removed"));
    }

    #[test]
    fn a_new_task_uses_the_chosen_account_else_the_default_else_the_built_in_one() {
        let db = db();
        brachio_store::add_agent_account(&db, "x1", "codex", "A", 1).unwrap();
        brachio_store::add_agent_account(&db, "x2", "codex", "B", 2).unwrap();
        brachio_store::add_agent_account(&db, "c1", "claude_code", "C", 3).unwrap();
        let pick = |kind, choice| resolve_choice(&db, kind, choice);
        assert_eq!(pick(AgentKind::Codex, None).unwrap(), None);
        brachio_store::set_default_agent_account(&db, "codex", Some("x2")).unwrap();
        assert_eq!(pick(AgentKind::Codex, None).unwrap().as_deref(), Some("x2"));
        assert_eq!(
            pick(AgentKind::Codex, Some("x1")).unwrap().as_deref(),
            Some("x1")
        );
        assert_eq!(pick(AgentKind::Codex, Some(BUILT_IN)).unwrap(), None);
        assert_eq!(pick(AgentKind::ClaudeCode, None).unwrap(), None);
        assert!(pick(AgentKind::Codex, Some("c1")).is_err());
        assert!(pick(AgentKind::Codex, Some("gone")).is_err());
        assert_eq!(pick(AgentKind::Grok, None).unwrap(), None);
    }
}
