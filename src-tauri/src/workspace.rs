//! IPC commands for repositories and tasks: add a repository, create a task
//! with its own worktree and branch, and list what is stored.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use brachio_agent::AgentKind;
use brachio_git::{AddWorktree, Git};
use brachio_store::{NewRepo, NewTask, RepoRecord, TaskRecord};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

/// App state that owns the database connection.
pub struct Database(pub Mutex<Connection>);

impl Database {
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.0
            .lock()
            .map_err(|_| "database lock poisoned".to_owned())
    }
}

/// A new task from the "New task" dialog.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    /// Repository to branch from.
    pub repo_id: String,
    /// Task title. The branch name is derived from it.
    pub title: String,
    /// Agent to run in the task.
    pub agent: AgentKind,
    /// Commit-ish to branch from. Defaults to the repository's default branch.
    pub base: Option<String>,
}

/// Lists registered repositories.
#[tauri::command]
pub fn repo_list(database: State<'_, Database>) -> Result<Vec<RepoRecord>, String> {
    RepoRecord::list(&*database.lock()?).map_err(to_text)
}

/// Adds the repository that contains `path`. Adding it twice returns the
/// existing entry.
#[tauri::command]
pub fn repo_add(database: State<'_, Database>, path: PathBuf) -> Result<RepoRecord, String> {
    let info = brachio_git::inspect_repo(&path).map_err(to_text)?;
    let connection = database.lock()?;
    if let Some(existing) = RepoRecord::find_by_path(&connection, &info.root).map_err(to_text)? {
        return Ok(existing);
    }
    let id = new_id();
    let repo = NewRepo {
        id: &id,
        name: &folder_name(&info.root),
        path: &info.root,
        default_branch: &info.default_branch,
        added_at_ms: now_ms(),
    };
    RepoRecord::insert(&connection, &repo).map_err(to_text)
}

/// Removes a repository from Brachio. Files on disk are not touched.
#[tauri::command]
pub fn repo_remove(database: State<'_, Database>, id: String) -> Result<(), String> {
    let connection = database.lock()?;
    let tasks = TaskRecord::count_for_repo(&connection, &id).map_err(to_text)?;
    if tasks > 0 {
        return Err(format!("the repository still has {tasks} tasks"));
    }
    RepoRecord::delete(&connection, &id).map_err(to_text)
}

/// Lists tasks that are not archived, newest first.
#[tauri::command]
pub fn task_list(database: State<'_, Database>) -> Result<Vec<TaskRecord>, String> {
    TaskRecord::list_open(&*database.lock()?).map_err(to_text)
}

/// Creates a worktree on a fresh branch and stores the task.
///
/// git runs off the UI thread, since `worktree add` checks out files.
#[tauri::command]
pub async fn task_create(
    database: State<'_, Database>,
    request: CreateTaskRequest,
) -> Result<TaskRecord, String> {
    let repo = RepoRecord::get(&*database.lock()?, &request.repo_id).map_err(to_text)?;
    let id = new_id();
    let branch = brachio_git::branch_name(&request.title, &id[..6]);
    let path = brachio_git::worktree_path(&repo.path, &branch);
    let add = AddWorktree {
        branch: branch.clone(),
        path: path.clone(),
        base: request
            .base
            .clone()
            .unwrap_or_else(|| repo.default_branch.clone()),
    };
    let repo_path = repo.path.clone();
    let added = tauri::async_runtime::spawn_blocking(move || {
        brachio_git::add_worktree(&Git::new(&repo_path), &add)
    });
    added.await.map_err(to_text)?.map_err(to_text)?;

    // Pinning the conversation id lets the task resume exactly its own
    // conversation after a restart.
    let session_id = request
        .agent
        .accepts_session_id()
        .then(|| uuid::Uuid::new_v4().hyphenated().to_string());
    let task = NewTask {
        id: &id,
        repo_id: &repo.id,
        title: request.title.trim(),
        branch: &branch,
        worktree_path: &path,
        agent_kind: Some(agent_name(request.agent)),
        agent_session_id: session_id.as_deref(),
        created_at_ms: now_ms(),
    };
    TaskRecord::insert(&*database.lock()?, &task).map_err(to_text)
}

/// Hides a task from the list. The worktree and branch stay on disk.
#[tauri::command]
pub fn task_archive(database: State<'_, Database>, id: String) -> Result<(), String> {
    TaskRecord::archive(&*database.lock()?, &id, now_ms()).map_err(to_text)
}

/// Changes in a task worktree relative to its base.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskChanges {
    patch: String,
    untracked: Vec<String>,
    truncated: bool,
}

/// Reads branch and working-tree changes without modifying the repository.
#[tauri::command]
pub async fn task_changes(
    database: State<'_, Database>,
    id: String,
) -> Result<TaskChanges, String> {
    let (task, repo) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &id).map_err(to_text)?;
        let repo = RepoRecord::get(&connection, &task.repo_id).map_err(to_text)?;
        (task, repo)
    };
    tauri::async_runtime::spawn_blocking(move || {
        const PATCH_LIMIT: usize = 256 * 1024;
        let git = Git::new(&task.worktree_path);
        let base_ref = format!("refs/heads/{}", repo.default_branch);
        let base = git
            .run(&["merge-base", "HEAD", &base_ref])
            .map_err(to_text)?;
        let mut patch = git
            .run_text(&[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                &base,
                "--",
            ])
            .map_err(to_text)?;
        let truncated = patch.len() > PATCH_LIMIT;
        if truncated {
            let mut end = PATCH_LIMIT;
            while !patch.is_char_boundary(end) {
                end -= 1;
            }
            patch.truncate(end);
        }
        let untracked = git
            .run_text(&["ls-files", "--others", "--exclude-standard", "-z"])
            .map_err(to_text)?
            .split_terminator('\0')
            .map(str::to_owned)
            .collect();
        Ok(TaskChanges {
            patch,
            untracked,
            truncated,
        })
    })
    .await
    .map_err(to_text)?
}

/// Errors cross the IPC boundary as display strings.
fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn agent_name(agent: AgentKind) -> &'static str {
    match agent {
        AgentKind::ClaudeCode => "claude_code",
        AgentKind::Codex => "codex",
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}
