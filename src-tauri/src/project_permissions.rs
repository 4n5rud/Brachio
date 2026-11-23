//! Looks up a project's `[permissions]` for the approval bar and for the project settings.

use brachio_agent::{read_policy, PermissionPolicy};
use brachio_store::{RepoRecord, TaskRecord};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::always_allow::task_of;
use crate::hook_socket::PolicyAnswer;
use crate::workspace::Database;

/// The rules of one project, as the settings show them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsView {
    /// Tools answered without asking.
    allow: Vec<String>,
    /// Tools or paths that are refused.
    deny: Vec<String>,
    /// Why the rules cannot be used. Everything is asked then.
    error: Option<String>,
}

/// What the project's rules say about a request from the terminal `key`.
pub(crate) fn answer_for(
    connection: &Connection,
    key: &str,
    tool: &str,
    input: &Value,
) -> PolicyAnswer {
    let found = TaskRecord::get(connection, task_of(key))
        .and_then(|task| RepoRecord::get(connection, &task.repo_id).map(|repo| (task, repo)));
    let Ok((task, repo)) = found else {
        return PolicyAnswer::Decided(brachio_agent::PolicyDecision::Ask);
    };
    match read_policy(&repo.path) {
        Ok(policy) => PolicyAnswer::Decided(policy.decide(tool, input, &task.worktree_path)),
        Err(error) => PolicyAnswer::Unusable(format!(
            "The project's permission rules were not used, so everything is asked. {error}"
        )),
    }
}

pub(crate) fn answer(app: &AppHandle, key: &str, tool: &str, input: &Value) -> PolicyAnswer {
    let database = app.state::<Database>();
    let Ok(connection) = database.lock() else {
        return PolicyAnswer::Decided(brachio_agent::PolicyDecision::Ask);
    };
    answer_for(&connection, key, tool, input)
}

fn view(policy: Result<PermissionPolicy, String>) -> PermissionsView {
    match policy {
        Ok(policy) => PermissionsView {
            allow: policy.allowed().to_vec(),
            deny: policy.denied(),
            error: None,
        },
        Err(error) => PermissionsView {
            allow: Vec::new(),
            deny: Vec::new(),
            error: Some(error),
        },
    }
}

/// The `[permissions]` rules of a project, or the reason they are not in force.
#[tauri::command]
pub fn repo_permissions(
    database: State<'_, Database>,
    repo_id: String,
) -> Result<PermissionsView, String> {
    let repo = RepoRecord::get(&*database.lock()?, &repo_id).map_err(|e| e.to_string())?;
    Ok(view(read_policy(&repo.path)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use brachio_agent::PolicyDecision;
    use brachio_store::open_in_memory;
    use serde_json::json;

    use super::*;

    fn project(brachio_toml: Option<&str>) -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        if let Some(text) = brachio_toml {
            std::fs::write(dir.path().join("brachio.toml"), text).unwrap();
        }
        let db = open_in_memory().unwrap();
        let path = dir.path().to_str().unwrap();
        db.execute(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', ?1, 'main')",
            [path],
        )
        .unwrap();
        db.execute(
            "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('t', 'r', 'T', '', 'b', ?1, 'new', 0)",
            [path],
        )
        .unwrap();
        (dir, db)
    }

    fn decided(answer: PolicyAnswer) -> PolicyDecision {
        match answer {
            PolicyAnswer::Decided(decision) => decision,
            PolicyAnswer::Unusable(note) => panic!("unusable: {note}"),
        }
    }

    #[test]
    fn a_project_without_rules_asks_for_everything() {
        for text in [None, Some(""), Some("[permissions]")] {
            let (_dir, db) = project(text);
            for tool in ["Read", "Bash", "Write"] {
                let input = json!({"file_path": "a.txt", "command": "ls"});
                let answer = answer_for(&db, "t:agent", tool, &input);
                assert_eq!(decided(answer), PolicyDecision::Ask, "{tool}");
            }
        }
    }

    #[test]
    fn the_projects_rules_allow_and_deny_requests_of_its_tasks() {
        let (_dir, db) = project(Some(
            "[permissions]\nallow = [\"Read\"]\ndeny = [\"Write:.env\"]",
        ));
        let read = json!({"file_path": "src/a.rs"});
        assert!(matches!(
            decided(answer_for(&db, "t:agent", "Read", &read)),
            PolicyDecision::Allow { .. }
        ));
        let write = json!({"file_path": ".env", "content": "x"});
        assert!(matches!(
            decided(answer_for(&db, "t:agent", "Write", &write)),
            PolicyDecision::Deny { .. }
        ));
        let bash = json!({"command": "ls"});
        assert_eq!(
            decided(answer_for(&db, "t:agent", "Bash", &bash)),
            PolicyDecision::Ask
        );
    }

    #[test]
    fn rules_that_cannot_be_read_ask_for_everything_and_say_why() {
        let (_dir, db) = project(Some("[permissions]\nallow = [\"Bash\"]"));
        let answer = answer_for(&db, "t:agent", "Read", &json!({"file_path": "a"}));
        assert!(
            matches!(answer, PolicyAnswer::Unusable(note) if note.contains("not a read-only tool"))
        );
    }
}
