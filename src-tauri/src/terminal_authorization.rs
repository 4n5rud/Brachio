//! Resolves trusted task and tab metadata before opening a terminal.

use brachio_agent::AgentKind;
use brachio_store::{TabKind, TaskRecord};
use rusqlite::Connection;

pub(crate) struct TerminalOwner {
    pub task: TaskRecord,
    pub agent: Option<AgentKind>,
    pub conversation: Option<String>,
    /// The detected shell a shell tab runs; `None` runs the default shell.
    pub shell: Option<String>,
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
        None => AgentKind::ClaudeCode,
        Some(id) => {
            AgentKind::from_id(id).ok_or_else(|| "The task has an unsupported agent.".to_owned())?
        }
    };
    let (agent, conversation, shell) = match pane {
        "shell" => (None, None, None),
        "agent" => (Some(primary), task.agent_session_id.clone(), None),
        _ => {
            let (is_agent, requested) = pane
                .strip_prefix("agent-")
                .map(|id| (true, id))
                .or_else(|| pane.strip_prefix("shell-").map(|id| (false, id)))
                .filter(|(_, id)| uuid::Uuid::parse_str(id).is_ok())
                .ok_or_else(|| "Unknown task terminal.".to_owned())?;
            let tab = brachio_store::task_tab(connection, task_id, requested)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "The tab is closed.".to_owned())?;
            match (is_agent, tab.kind) {
                (true, TabKind::ClaudeCode) => (
                    Some(AgentKind::ClaudeCode),
                    Some(requested.to_owned()),
                    None,
                ),
                (true, TabKind::Codex) => (Some(AgentKind::Codex), None, None),
                (false, TabKind::Shell) => (None, None, tab.shell_id),
                _ => return Err("The tab does not match its terminal.".to_owned()),
            }
        }
    };
    Ok(TerminalOwner {
        task,
        agent,
        conversation,
        shell,
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
        let added = |kind, id: &str| brachio_store::TaskTab {
            tab_id: id.to_owned(),
            kind,
            shell_id: None,
        };
        brachio_store::add_task_tab(&db, "task", &added(TabKind::ClaudeCode, tab))
            .expect("register tab");
        let owner = resolve(&db, &key).expect("registered tab");
        assert_eq!(owner.agent, Some(AgentKind::ClaudeCode));
        assert_eq!(owner.conversation.as_deref(), Some(tab));
        brachio_store::close_task_tab(&db, "task", tab).expect("close tab");
        assert!(resolve(&db, &key).is_err());
    }

    #[test]
    fn two_sessions_of_the_same_agent_in_one_task_each_resolve_on_their_own() {
        let db = brachio_store::open_in_memory().expect("test database");
        db.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', 'repo', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, agent_kind)
             VALUES ('task', 'r', 'Task', '', 'task', 'worktree', 'active', 1, 'codex');",
        ).expect("test task");
        let first = "11111111-1111-4111-8111-111111111111";
        let second = "22222222-2222-4222-8222-222222222222";
        let shell = "33333333-3333-4333-8333-333333333333";
        for (kind, id) in [
            (TabKind::Codex, first),
            (TabKind::Codex, second),
            (TabKind::Shell, shell),
        ] {
            let tab = brachio_store::TaskTab {
                tab_id: id.to_owned(),
                kind,
                shell_id: None,
            };
            brachio_store::add_task_tab(&db, "task", &tab).expect("register tab");
        }
        assert_eq!(
            resolve(&db, "task:agent").expect("primary").agent,
            Some(AgentKind::Codex)
        );
        assert_eq!(
            resolve(&db, &format!("task:agent-{first}"))
                .expect("first")
                .agent,
            Some(AgentKind::Codex)
        );
        assert_eq!(
            resolve(&db, &format!("task:agent-{second}"))
                .expect("second")
                .agent,
            Some(AgentKind::Codex)
        );
        assert_eq!(
            resolve(&db, &format!("task:shell-{shell}"))
                .expect("shell")
                .agent,
            None
        );
        assert!(resolve(&db, &format!("task:agent-{shell}")).is_err());
        brachio_store::close_task_tab(&db, "task", first).expect("close first");
        assert!(resolve(&db, &format!("task:agent-{first}")).is_err());
        assert!(resolve(&db, &format!("task:agent-{second}")).is_ok());
    }

    #[test]
    fn a_grok_task_resolves_to_grok_and_an_unknown_agent_is_refused() {
        let db = brachio_store::open_in_memory().expect("test database");
        db.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', 'repo', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, agent_kind)
             VALUES ('grok', 'r', 'Task', '', 'a', 'worktree', 'active', 1, 'grok'),
                    ('odd', 'r', 'Task', '', 'b', 'worktree', 'active', 1, 'future_agent');",
        ).expect("test tasks");
        assert_eq!(
            resolve(&db, "grok:agent").expect("grok").agent,
            Some(AgentKind::Grok)
        );
        assert!(resolve(&db, "odd:agent").is_err());
    }
}
