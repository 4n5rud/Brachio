//! Restore points for a task's worktree: captured when an agent turn starts and ends, listed and
//! diffed in the Changes panel, and restored only on request. They hold files only; the agent's
//! conversation is not part of them.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use brachio_git::{CaptureKind, Git};
use brachio_store::{RepoRecord, TaskRecord};
use serde::Serialize;
use tauri::State;

use crate::workspace::{lock_open_git_task, Database, TaskWorktreeLocks};

fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Which tasks have an agent mid-turn, as the UI last reported. A restore is refused for these.
#[derive(Default)]
pub struct TurnTracker(Mutex<HashSet<String>>);

impl TurnTracker {
    fn set_active(&self, task_id: &str, active: bool) -> Result<(), String> {
        let mut tasks = self
            .0
            .lock()
            .map_err(|_| "turn tracker poisoned".to_owned())?;
        if active {
            tasks.insert(task_id.to_owned());
        } else {
            tasks.remove(task_id);
        }
        Ok(())
    }

    fn is_active(&self, task_id: &str) -> Result<bool, String> {
        let tasks = self
            .0
            .lock()
            .map_err(|_| "turn tracker poisoned".to_owned())?;
        Ok(tasks.contains(task_id))
    }
}

/// One restore point as the panel shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointView {
    number: u32,
    created_at_ms: i64,
    kind: String,
    files: u32,
    additions: u32,
    deletions: u32,
}

/// The patch between a restore point and the one before it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointPatch {
    patch: String,
    truncated: bool,
    base_number: Option<u32>,
}

/// What a restore left behind for undoing it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointRestored {
    backup_number: u32,
}

/// The worktree of an open task in a git project, or none for an archived or folder task.
pub(crate) fn git_worktree(database: &Database, id: &str) -> Result<Option<PathBuf>, String> {
    let connection = database.lock()?;
    let task = TaskRecord::get(&connection, id).map_err(to_text)?;
    let repo = RepoRecord::get(&connection, &task.repo_id).map_err(to_text)?;
    let usable = repo.is_git && task.archived_at_ms.is_none();
    Ok(usable.then_some(task.worktree_path))
}

pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, brachio_git::GitError> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(to_text)?
        .map_err(to_text)
}

/// Records that the agent started or finished a turn and snapshots the worktree. Returns whether a
/// new restore point was stored; nothing is stored when no file changed.
#[tauri::command]
pub async fn task_checkpoint_turn(
    database: State<'_, Database>,
    locks: State<'_, TaskWorktreeLocks>,
    tracker: State<'_, TurnTracker>,
    id: String,
    active: bool,
) -> Result<bool, String> {
    tracker.set_active(&id, active)?;
    let Some(worktree) = git_worktree(&database, &id)? else {
        return Ok(false);
    };
    if !worktree.is_dir() {
        return Ok(false);
    }
    let _guard = locks.acquire(&format!("checkpoint:{id}")).await?;
    let kind = if active {
        CaptureKind::TurnStart
    } else {
        CaptureKind::TurnEnd
    };
    let created =
        blocking(move || brachio_git::capture_checkpoint(&Git::new(&worktree), &id, kind)).await?;
    Ok(created.is_some())
}

/// A task's restore points, oldest first.
#[tauri::command]
pub async fn task_checkpoint_list(
    database: State<'_, Database>,
    id: String,
) -> Result<Vec<CheckpointView>, String> {
    let Some(worktree) = git_worktree(&database, &id)? else {
        return Ok(Vec::new());
    };
    let points = blocking(move || brachio_git::list_checkpoints(&Git::new(&worktree), &id)).await?;
    Ok(points
        .into_iter()
        .map(|point| CheckpointView {
            number: point.number,
            created_at_ms: point.created_at_secs.saturating_mul(1000),
            kind: point.kind,
            files: point.files,
            additions: point.additions,
            deletions: point.deletions,
        })
        .collect())
}

/// What one restore point changed compared with the one before it.
#[tauri::command]
pub async fn task_checkpoint_diff(
    database: State<'_, Database>,
    id: String,
    number: u32,
) -> Result<CheckpointPatch, String> {
    let Some(worktree) = git_worktree(&database, &id)? else {
        return Err("Restore points need a git task.".to_owned());
    };
    let diff =
        blocking(move || brachio_git::checkpoint_diff(&Git::new(&worktree), &id, number)).await?;
    Ok(CheckpointPatch {
        patch: diff.patch,
        truncated: diff.truncated,
        base_number: diff.base_number,
    })
}

/// Puts the task's files back as a restore point has them, after taking a backup point. The UI
/// has asked the user to confirm. Refused while the agent is working.
#[tauri::command]
pub async fn task_checkpoint_restore(
    database: State<'_, Database>,
    locks: State<'_, TaskWorktreeLocks>,
    tracker: State<'_, TurnTracker>,
    id: String,
    number: u32,
) -> Result<CheckpointRestored, String> {
    if tracker.is_active(&id)? {
        return Err("The agent is working. Wait for the turn to finish, then restore.".to_owned());
    }
    let _task_guard = locks.acquire(&format!("checkpoint:{id}")).await?;
    let (task, _repo_guard) = lock_open_git_task(&database, &locks, &id).await?;
    if tracker.is_active(&id)? {
        return Err("The agent is working. Wait for the turn to finish, then restore.".to_owned());
    }
    let worktree = task.worktree_path;
    let restored = blocking(move || {
        brachio_git::restore_checkpoint(&Git::new(&worktree), Path::new(&worktree), &id, number)
    })
    .await?;
    Ok(CheckpointRestored {
        backup_number: restored.backup_number,
    })
}

/// Drops all of a task's restore points, for when it is archived or deleted. A failure only
/// leaves hidden refs behind, so it is logged and not raised.
pub(crate) fn forget(git: &Git, task_id: &str) {
    if let Err(error) = brachio_git::delete_checkpoints(git, task_id) {
        tracing::warn!(%error, task_id, "could not remove restore points");
    }
}

/// `forget` for a task looked up by id, run from commands that have no repository path yet.
pub(crate) fn forget_for_task(database: &Database, task_id: &str) {
    let path = database.lock().ok().and_then(|connection| {
        let task = TaskRecord::get(&connection, task_id).ok()?;
        let repo = RepoRecord::get(&connection, &task.repo_id).ok()?;
        repo.is_git.then_some(repo.path)
    });
    if let Some(path) = path {
        forget(&Git::new(&path), task_id);
    }
}
