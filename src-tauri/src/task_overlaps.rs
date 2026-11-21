//! Files that two open tasks of one project both change.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use brachio_git::{changed_paths, shared_paths, Git};
use brachio_store::{RepoRecord, TaskRecord};
use serde::Serialize;
use tauri::State;

use crate::workspace::Database;

/// Changed paths are reused this long, so opening several tasks in a row runs git once each.
const CACHE_TTL: Duration = Duration::from_secs(5);

type CachedPaths = HashMap<String, (Instant, Vec<String>)>;

#[derive(Default, Clone)]
pub struct ChangedPathCache(Arc<Mutex<CachedPaths>>);

impl ChangedPathCache {
    fn fresh(&self, task_id: &str) -> Option<Vec<String>> {
        let cache = self.0.lock().ok()?;
        let (at, paths) = cache.get(task_id)?;
        (at.elapsed() < CACHE_TTL).then(|| paths.clone())
    }

    fn store(&self, task_id: &str, paths: Vec<String>) {
        if let Ok(mut cache) = self.0.lock() {
            cache.insert(task_id.to_owned(), (Instant::now(), paths));
        }
    }

    fn forget(&self, task_id: &str) {
        if let Ok(mut cache) = self.0.lock() {
            cache.remove(task_id);
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOverlap {
    task_id: String,
    title: String,
    files: Vec<String>,
}

fn paths_of(
    cache: &ChangedPathCache,
    task: &TaskRecord,
    default_branch: &str,
    refresh: bool,
) -> Option<Vec<String>> {
    if refresh {
        cache.forget(&task.id);
    } else if let Some(paths) = cache.fresh(&task.id) {
        return Some(paths);
    }
    let git = Git::new(&task.worktree_path);
    let base = git
        .run(&[
            "merge-base",
            "HEAD",
            &format!("refs/heads/{default_branch}"),
        ])
        .ok()?;
    let paths = changed_paths(&git, &base).ok()?;
    cache.store(&task.id, paths.clone());
    Some(paths)
}

/// Other open tasks of the same project that change files this task also changes.
/// `refresh` skips the short cache, for when the task's own changes were just re-read.
#[tauri::command]
pub async fn task_overlaps(
    database: State<'_, Database>,
    cache: State<'_, ChangedPathCache>,
    id: String,
    refresh: bool,
) -> Result<Vec<TaskOverlap>, String> {
    let (task, repo, others) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &id).map_err(|error| error.to_string())?;
        let repo =
            RepoRecord::get(&connection, &task.repo_id).map_err(|error| error.to_string())?;
        let others: Vec<TaskRecord> = TaskRecord::list_open(&connection)
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|other| other.repo_id == task.repo_id && other.id != task.id)
            .collect();
        (task, repo, others)
    };
    if !repo.is_git || others.is_empty() {
        return Ok(Vec::new());
    }
    let cache = cache.inner().clone();
    let overlaps = tauri::async_runtime::spawn_blocking(move || {
        let ours = paths_of(&cache, &task, &repo.default_branch, refresh)?;
        let mut overlaps = Vec::new();
        for other in &others {
            let Some(theirs) = paths_of(&cache, other, &repo.default_branch, false) else {
                continue;
            };
            let files = shared_paths(&ours, &theirs);
            if !files.is_empty() {
                overlaps.push(TaskOverlap {
                    task_id: other.id.clone(),
                    title: other.title.clone(),
                    files,
                });
            }
        }
        Some(overlaps)
    })
    .await
    .map_err(|error| error.to_string())?;
    Ok(overlaps.unwrap_or_default())
}
