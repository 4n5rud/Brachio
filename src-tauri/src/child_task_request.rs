//! Child tasks that an agent asks for with `brachio task create`: what is checked before the
//! user is asked, and how the task is made once they agree.

use std::future::Future;
use std::pin::Pin;

use brachio_agent::AgentKind;
use brachio_core::task_graph::TaskState;
use brachio_store::{RepoRecord, TaskRecord};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

use crate::mailbox::agent_owner;
use crate::workspace::{self, CreateTaskRequest, Database, TaskWorktreeLocks};

/// Tells the interface a task was created at an agent's request. The payload is a
/// `workspace::CreatedTask`, the same object `task_create` returns.
pub const CREATED_EVENT: &str = "task://created";
pub const MAX_OPEN_CHILDREN: i64 = 4;
const MAX_TITLE_CHARS: usize = 200;
const MAX_PROMPT_BYTES: usize = 8 * 1024;
const DEFAULT_AGENT: &str = "claude";

/// What an agent asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRequest {
    pub key: String,
    pub title: String,
    pub prompt: String,
    pub agent: Option<String>,
    pub base: Option<String>,
    pub after: Vec<String>,
}

impl ChildRequest {
    /// The agent name the user is shown.
    pub fn agent_name(&self) -> &str {
        self.agent.as_deref().unwrap_or(DEFAULT_AGENT)
    }
}

/// The task that now exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildTask {
    pub key: String,
    pub title: String,
    pub branch: String,
}

pub type CreateFuture = Pin<Box<dyn Future<Output = Result<ChildTask, String>> + Send>>;

/// How the socket checks and carries out a request for a child task.
pub trait ChildTasks: Send + Sync {
    /// Returns the id of the asking task, or why the request is refused.
    fn check(&self, request: &ChildRequest) -> Result<String, String>;
    /// Creates the child of `lead_id`.
    fn create(&self, lead_id: String, request: ChildRequest) -> CreateFuture;
}

fn agent_kind(name: &str) -> Result<AgentKind, String> {
    match name {
        "claude" => Ok(AgentKind::ClaudeCode),
        "codex" => Ok(AgentKind::Codex),
        _ => Err("The agent must be claude or codex.".to_owned()),
    }
}

pub(crate) fn is_hidden_or_control(character: char) -> bool {
    (character.is_control() && character != '\n' && character != '\t')
        || matches!(
            character,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

/// Refuses requests that should not reach the user, and returns the asking task.
pub fn check_request(
    connection: &Connection,
    request: &ChildRequest,
) -> Result<TaskRecord, String> {
    let lead = agent_owner(connection, &request.key)?;
    if lead.parent_task_id.is_some() {
        return Err("A task that was created from another task cannot create tasks.".to_owned());
    }
    let repo = RepoRecord::get(connection, &lead.repo_id).map_err(|error| error.to_string())?;
    if !repo.is_git {
        return Err("Tasks in a plain folder cannot create tasks.".to_owned());
    }
    let open_children: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE parent_task_id = ?1 AND archived_at_ms IS NULL",
            [&lead.id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if open_children >= MAX_OPEN_CHILDREN {
        return Err(format!(
            "This task already has {MAX_OPEN_CHILDREN} open child tasks. Archive one first."
        ));
    }
    if request.title.trim().is_empty() || request.title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!(
            "The title must be 1 to {MAX_TITLE_CHARS} characters."
        ));
    }
    if request.prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!(
            "The prompt must be at most {} KiB.",
            MAX_PROMPT_BYTES / 1024
        ));
    }
    agent_kind(request.agent_name())?;
    check_after(connection, &lead.id, &request.after)?;
    let shown = [&request.title, &request.prompt]
        .into_iter()
        .chain(request.base.as_ref());
    if shown
        .flat_map(|text| text.chars())
        .any(is_hidden_or_control)
    {
        return Err(
            "The text contains control or invisible characters, which are not allowed.".to_owned(),
        );
    }
    if request
        .base
        .as_deref()
        .is_some_and(|base| base.starts_with('-'))
    {
        return Err("The base cannot start with a dash.".to_owned());
    }
    Ok(lead)
}

/// The ids of the open sibling tasks a new child waits for. Waiting needs the lead's scheduler,
/// and a predecessor that already failed would leave the child waiting forever.
fn check_after(connection: &Connection, lead_id: &str, after: &[String]) -> Result<(), String> {
    if after.is_empty() {
        return Ok(());
    }
    if after.len() > MAX_OPEN_CHILDREN as usize {
        return Err(format!(
            "A task can wait for at most {MAX_OPEN_CHILDREN} others."
        ));
    }
    if !brachio_store::scheduler_enabled(connection, lead_id).map_err(|e| e.to_string())? {
        return Err(
            "Turn on \"Start automatically\" for this task in the Tasks tab before using --after."
                .to_owned(),
        );
    }
    let graph = brachio_store::load_graph(connection, lead_id).map_err(|e| e.to_string())?;
    for key in after {
        let id = after_task_id(key);
        let open: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND parent_task_id = ?2
                               AND archived_at_ms IS NULL)",
                [id, lead_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !open {
            return Err(format!("{key} is not an open child task of this task."));
        }
        let state = graph.state(id).map_err(|e| e.to_string())?;
        if matches!(
            state,
            TaskState::Failed | TaskState::Cancelled | TaskState::Blocked
        ) {
            return Err(format!(
                "{key} is {state}, so a task waiting for it could not run."
            ));
        }
    }
    Ok(())
}

fn after_task_id(key: &str) -> &str {
    key.split_once(':').map_or(key, |(id, _)| id)
}

/// The app's implementation: the real database, worktrees and window.
pub struct AppChildTasks(pub AppHandle);

impl ChildTasks for AppChildTasks {
    fn check(&self, request: &ChildRequest) -> Result<String, String> {
        let database = self.0.state::<Database>();
        let connection = database.lock()?;
        check_request(&connection, request).map(|lead| lead.id)
    }

    fn create(&self, lead_id: String, request: ChildRequest) -> CreateFuture {
        let app = self.0.clone();
        Box::pin(async move {
            let agent = agent_kind(request.agent_name())?;
            let database = app.state::<Database>();
            let worktrees = app.state::<TaskWorktreeLocks>();
            let (repo_id, scheduled) = {
                let connection = database.lock()?;
                let repo_id = TaskRecord::get(&connection, &lead_id)
                    .map_err(|error| error.to_string())?
                    .repo_id;
                let scheduled = brachio_store::scheduler_enabled(&connection, &lead_id)
                    .map_err(|error| error.to_string())?;
                (repo_id, scheduled)
            };
            let created = workspace::create_task(
                &database,
                &worktrees,
                CreateTaskRequest {
                    repo_id,
                    scheduled,
                    after: request
                        .after
                        .iter()
                        .map(|key| after_task_id(key).to_owned())
                        .collect(),
                    parent_task_id: Some(lead_id),
                    title: request.title,
                    prompt: request.prompt,
                    agent,
                    base: request.base,
                    max_open_children: Some(MAX_OPEN_CHILDREN),
                },
            )
            .await?;
            let child = ChildTask {
                key: format!("{}:agent", created.task.id),
                title: created.task.title.clone(),
                branch: created.task.branch.clone(),
            };
            if scheduled {
                crate::orchestration::schedule_all(&app);
            } else if let Err(error) = app.emit(CREATED_EVENT, &created) {
                tracing::debug!(%error, "could not tell the window about the new task");
            }
            Ok(child)
        })
    }
}

#[cfg(test)]
mod tests {
    use brachio_store::open_in_memory;

    use super::{check_request, ChildRequest};

    fn database() -> rusqlite::Connection {
        let connection = open_in_memory().expect("open database");
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('lead1111', 'r', 'Lead', '', 'b0', '/w0', 'new', 0);",
            )
            .expect("insert lead");
        connection
    }

    fn add_child(connection: &rusqlite::Connection, id: &str) {
        connection
            .execute(
                "INSERT INTO tasks (id, repo_id, parent_task_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES (?1, 'r', 'lead1111', 'Child', '', ?1, ?1, 'new', 0)",
                [id],
            )
            .expect("insert child");
    }

    fn request(key: &str, title: &str) -> ChildRequest {
        ChildRequest {
            key: key.to_owned(),
            title: title.to_owned(),
            prompt: "do it".to_owned(),
            agent: None,
            base: None,
            after: Vec::new(),
        }
    }

    #[test]
    fn a_valid_request_returns_the_asking_task() {
        let connection = database();
        let lead = check_request(&connection, &request("lead1111:agent", "Write docs"));
        assert_eq!(lead.expect("allowed").id, "lead1111");
    }

    #[test]
    fn a_child_task_cannot_create_tasks() {
        let connection = database();
        add_child(&connection, "child001");
        assert!(check_request(&connection, &request("child001:agent", "Deeper")).is_err());
    }

    #[test]
    fn the_fifth_open_child_is_refused_until_one_is_archived() {
        let connection = database();
        for id in ["c1", "c2", "c3", "c4"] {
            add_child(&connection, id);
        }
        let ask = request("lead1111:agent", "One more");
        assert!(check_request(&connection, &ask).is_err());
        connection
            .execute("UPDATE tasks SET archived_at_ms = 5 WHERE id = 'c1'", [])
            .expect("archive");
        assert!(check_request(&connection, &ask).is_ok());
    }

    #[test]
    fn unknown_and_shell_keys_are_refused() {
        let connection = database();
        assert!(check_request(&connection, &request("ghost:agent", "x")).is_err());
        assert!(check_request(&connection, &request("lead1111:shell", "x")).is_err());
    }

    #[test]
    fn a_direction_changing_character_in_the_title_is_refused() {
        let connection = database();
        let ask = request("lead1111:agent", "Fix \u{202E}gpj.exe");
        assert!(check_request(&connection, &ask).is_err());
    }
}
