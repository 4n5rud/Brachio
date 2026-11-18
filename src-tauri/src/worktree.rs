//! IPC command that creates a task worktree on a fresh branch.

use std::path::PathBuf;

use brachio_git::{AddWorktree, Git};
use serde::{Deserialize, Serialize};

/// A new task's repository and title.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    /// Repository to branch from.
    pub repo: PathBuf,
    /// Task title. The branch name is derived from it.
    pub title: String,
    /// Commit-ish to branch from. Defaults to `HEAD`.
    pub base: Option<String>,
}

/// The worktree that was created.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    /// New branch name.
    pub branch: String,
    /// Worktree folder.
    pub path: PathBuf,
}

/// Creates a worktree off the UI thread, since `worktree add` checks out
/// files.
#[tauri::command]
pub async fn worktree_create(request: CreateRequest) -> Result<Created, String> {
    let task = tauri::async_runtime::spawn_blocking(move || create(&request));
    task.await.map_err(|error| error.to_string())?
}

fn create(request: &CreateRequest) -> Result<Created, String> {
    let branch = brachio_git::branch_name(&request.title, &short_id());
    let path = brachio_git::worktree_path(&request.repo, &branch);
    let add = AddWorktree {
        branch: branch.clone(),
        path: path.clone(),
        base: request.base.clone().unwrap_or_else(|| "HEAD".to_owned()),
    };
    brachio_git::add_worktree(&Git::new(&request.repo), &add).map_err(|error| error.to_string())?;
    Ok(Created { branch, path })
}

/// Six hex characters: enough to keep branch names apart, short to read.
fn short_id() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    id[..6].to_owned()
}
