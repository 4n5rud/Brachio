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
                (true, TabKind::Codex) => (
                    Some(AgentKind::Codex),
                    brachio_store::task_tab_conversation(connection, task_id, requested)
                        .map_err(|error| error.to_string())?,
                    None,
                ),
                (false, TabKind::Shell) => (None, None, tab.target),
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

/// The name of an agent terminal in the note its Codex launch carries, see
/// [`brachio_agent::codex_tab_note`]: the key with `:` turned into `-`, such as
/// `<task id>-agent-<tab id>`. `None` for a key that cannot name a tab.
pub(crate) fn tab_marker(key: &str) -> Option<String> {
    let marker = key.replacen(':', "-", 1);
    brachio_agent::is_tab_marker(&marker).then_some(marker)
}

/// Records the Codex conversation the Codex terminal `key` is to resume, in place of any recorded
/// before. Only for a conversation confirmed to be the tab's. Fails for any other terminal, so a
/// Claude Code tab's conversation id is never replaced.
pub(crate) fn record_codex_conversation(
    connection: &Connection,
    key: &str,
    conversation: &str,
) -> Result<(), String> {
    if resolve(connection, key)?.agent != Some(AgentKind::Codex) {
        return Err("Only a Codex tab records a Codex conversation.".to_owned());
    }
    let (task_id, pane) = key
        .split_once(':')
        .ok_or_else(|| "A registered task terminal is required.".to_owned())?;
    let recorded = match (pane, pane.strip_prefix("agent-")) {
        ("agent", _) => TaskRecord::set_agent_session(connection, task_id, conversation),
        (_, Some(tab)) => {
            brachio_store::set_task_tab_conversation(connection, task_id, tab, conversation)
        }
        _ => return Err("Only an agent terminal has a conversation.".to_owned()),
    };
    match recorded {
        Ok(true) => Ok(()),
        Ok(false) => Err("The tab is closed.".to_owned()),
        Err(error) => Err(error.to_string()),
    }
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
            target: None,
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
    fn a_found_codex_conversation_is_kept_by_its_own_tab_only() {
        let db = brachio_store::open_in_memory().expect("test database");
        db.execute_batch(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', 'repo', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, agent_kind)
             VALUES ('task', 'r', 'Task', '', 'task', 'worktree', 'active', 1, 'codex');",
        ).expect("test task");
        let first = "11111111-1111-4111-8111-111111111111";
        let second = "22222222-2222-4222-8222-222222222222";
        for id in [first, second] {
            let tab = brachio_store::TaskTab {
                tab_id: id.to_owned(),
                kind: TabKind::Codex,
                target: None,
            };
            brachio_store::add_task_tab(&db, "task", &tab).expect("register tab");
        }
        let conversation = |key: &str| resolve(&db, key).expect("resolve").conversation;
        assert_eq!(conversation("task:agent"), None);
        assert_eq!(conversation(&format!("task:agent-{first}")), None);

        record_codex_conversation(&db, "task:agent", "old-chat").expect("record");
        record_codex_conversation(&db, "task:agent", "main-chat").expect("replace");
        record_codex_conversation(&db, &format!("task:agent-{first}"), "first-chat")
            .expect("record");
        assert!(record_codex_conversation(&db, "task:shell", "x").is_err());
        let claude = "33333333-3333-4333-8333-333333333333";
        brachio_store::add_task_tab(
            &db,
            "task",
            &brachio_store::TaskTab {
                tab_id: claude.to_owned(),
                kind: TabKind::ClaudeCode,
                target: None,
            },
        )
        .expect("register tab");
        assert!(
            record_codex_conversation(&db, &format!("task:agent-{claude}"), "x").is_err(),
            "a Claude Code tab keeps its own id"
        );
        assert_eq!(conversation("task:agent").as_deref(), Some("main-chat"));
        assert_eq!(
            conversation(&format!("task:agent-{first}")).as_deref(),
            Some("first-chat")
        );
        assert_eq!(conversation(&format!("task:agent-{second}")), None);

        assert_eq!(tab_marker("task:agent").as_deref(), Some("task-agent"));
        assert_eq!(
            tab_marker(&format!("task:agent-{first}")),
            Some(format!("task-agent-{first}"))
        );
        assert_eq!(tab_marker("task:agent:x"), None);
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
                target: None,
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
