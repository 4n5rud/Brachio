//! Resolves trusted task and tab metadata before opening a terminal.

use brachio_agent::AgentKind;
use brachio_store::TaskRecord;
use rusqlite::{Connection, OptionalExtension};

pub(crate) struct TerminalOwner {
    pub task: TaskRecord,
    pub agent: Option<AgentKind>,
    pub conversation: Option<String>,
}

pub(crate) fn resolve(connection: &Connection, key: &str) -> Result<TerminalOwner, String> {
    let (task_id, pane) = key
        .split_once(':')
        .ok_or_else(|| "A registered task terminal is required.".to_owned())?;
    let task = TaskRecord::get(connection, task_id).map_err(|error| error.to_string())?;
    if task.archived_at_ms.is_some() {
        return Err("Restore the task before opening its terminal.".to_owned());
    }
    let primary = match task.agent_kind.as_deref() {
        Some("codex") => AgentKind::Codex,
        None | Some("claude_code") => AgentKind::ClaudeCode,
        Some(_) => return Err("The task has an unsupported agent.".to_owned()),
    };
    let (agent, conversation) = match pane {
        "shell" => (None, None),
        "agent" => (Some(primary), task.agent_session_id.clone()),
        _ => {
            let requested = pane
                .strip_prefix("agent-")
                .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                .ok_or_else(|| "Unknown task terminal.".to_owned())?;
            let registered = connection
                .query_row(
                    "SELECT tab_id FROM task_secondary_tabs WHERE task_id = ?1 AND tab_id IS NOT NULL",
                    [task_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            if registered.as_deref() != Some(requested) {
                return Err("The agent tab is closed or has been replaced.".to_owned());
            }
            match primary {
                AgentKind::ClaudeCode => (Some(AgentKind::Codex), None),
                AgentKind::Codex => (Some(AgentKind::ClaudeCode), registered),
            }
        }
    };
    Ok(TerminalOwner {
        task,
        agent,
        conversation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unregistered_and_replaced_tabs() {
        let db = brachio_store::open_in_memory().expect("test database");
        db.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', 'repo', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, agent_kind)
             VALUES ('task', 'r', 'Task', '', 'task', 'worktree', 'active', 1, 'codex');",
        ).expect("test task");
        let tab = "12345678-1234-4234-9234-123456789abc";
        let key = format!("task:agent-{tab}");
        assert!(resolve(&db, &key).is_err());
        assert!(resolve(&db, "missing:shell").is_err());
        assert_eq!(resolve(&db, "task:shell").expect("shell").agent, None);
        brachio_store::set_secondary_tab(&db, "task", Some(tab)).expect("register tab");
        let owner = resolve(&db, &key).expect("registered tab");
        assert_eq!(owner.agent, Some(AgentKind::ClaudeCode));
        assert_eq!(owner.conversation.as_deref(), Some(tab));
        brachio_store::set_secondary_tab(&db, "task", None).expect("close tab");
        assert!(resolve(&db, &key).is_err());
    }
}
