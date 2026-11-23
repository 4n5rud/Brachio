//! Read-only commit history of a task's branch, for the Changes panel.

use brachio_git::Git;
use serde::Serialize;
use tauri::State;

use crate::task_checkpoints::{blocking, git_worktree};
use crate::workspace::Database;

/// One commit as the history list shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitView {
    hash: String,
    parents: Vec<String>,
    author: String,
    committed_at_ms: i64,
    refs: Vec<String>,
    subject: String,
}

/// The patch of one commit against its first parent.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPatchView {
    patch: String,
    truncated: bool,
}

/// The commits of a task's branch, newest first. A folder task or an archived one has none.
#[tauri::command]
pub async fn task_commit_list(
    database: State<'_, Database>,
    id: String,
) -> Result<Vec<CommitView>, String> {
    let Some(worktree) = git_worktree(&database, &id)? else {
        return Ok(Vec::new());
    };
    if !worktree.is_dir() {
        return Ok(Vec::new());
    }
    let commits =
        blocking(move || brachio_git::list_commits(&Git::new(&worktree), brachio_git::MAX_COMMITS))
            .await?;
    Ok(commits
        .into_iter()
        .map(|commit| CommitView {
            hash: commit.hash,
            parents: commit.parents,
            author: commit.author,
            committed_at_ms: commit.time_secs.saturating_mul(1000),
            refs: commit.refs,
            subject: commit.subject,
        })
        .collect())
}

/// What one commit changed.
#[tauri::command]
pub async fn task_commit_diff(
    database: State<'_, Database>,
    id: String,
    hash: String,
) -> Result<CommitPatchView, String> {
    let Some(worktree) = git_worktree(&database, &id)? else {
        return Err("Commit history needs a git task.".to_owned());
    };
    let patch = blocking(move || brachio_git::commit_patch(&Git::new(&worktree), &hash)).await?;
    Ok(CommitPatchView {
        patch: patch.patch,
        truncated: patch.truncated,
    })
}
