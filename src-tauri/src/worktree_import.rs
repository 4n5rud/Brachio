//! Takes over worktrees git already has for a project as tasks, without touching their files.

use std::path::PathBuf;

use brachio_git::{importable, list_worktrees, select_importable, Git};
use brachio_store::{NewTask, RepoRecord, TaskRecord};
use serde::Serialize;
use tauri::State;

use crate::workspace::{agent_name, new_id, now_ms, Database, TaskWorktreeLocks};

/// A worktree of the project that Brachio does not track yet.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    path: String,
    branch: String,
}

fn project_and_known_paths(
    database: &Database,
    repo_id: &str,
) -> Result<(RepoRecord, Vec<PathBuf>), String> {
    let connection = database.lock()?;
    let repo = RepoRecord::get(&connection, repo_id).map_err(|error| error.to_string())?;
    if !repo.is_git {
        return Err("Only Git projects have worktrees to import.".to_owned());
    }
    let mut known: Vec<PathBuf> = TaskRecord::list_open(&connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .chain(TaskRecord::list_archived(&connection).map_err(|error| error.to_string())?)
        .map(|task| task.worktree_path)
        .collect();
    known.push(repo.path.clone());
    Ok((repo, known))
}

/// Worktrees of the project that no task uses yet.
#[tauri::command]
pub async fn worktree_import_candidates(
    database: State<'_, Database>,
    repo_id: String,
) -> Result<Vec<ImportCandidate>, String> {
    let (repo, known) = project_and_known_paths(&database, &repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let entries = list_worktrees(&Git::new(&repo.path)).map_err(|error| error.to_string())?;
        Ok(importable(&entries, &known)
            .into_iter()
            .filter_map(|entry| {
                Some(ImportCandidate {
                    path: entry.path.to_string_lossy().into_owned(),
                    branch: entry.branch?,
                })
            })
            .collect())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Adds one task for each chosen worktree. The list is read again from git, so only worktrees of
/// this project are accepted, and either every task is added or none is.
#[tauri::command]
pub async fn worktree_import(
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<Vec<TaskRecord>, String> {
    let _guard = worktrees.acquire(&repo_id).await?;
    let (repo, known) = project_and_known_paths(&database, &repo_id)?;
    let requested: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let chosen = tauri::async_runtime::spawn_blocking({
        let repo_path = repo.path.clone();
        move || {
            let entries =
                list_worktrees(&Git::new(&repo_path)).map_err(|error| error.to_string())?;
            select_importable(&entries, &known, &requested).map_err(|error| error.to_string())
        }
    })
    .await
    .map_err(|error| error.to_string())??;
    let mut connection = database.lock()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let mut records = Vec::new();
    for entry in &chosen {
        let Some(branch) = entry.branch.as_deref() else {
            continue;
        };
        let id = new_id();
        let task = NewTask {
            id: &id,
            repo_id: &repo.id,
            parent_task_id: None,
            title: branch,
            prompt: "",
            branch,
            worktree_path: &entry.path,
            agent_kind: Some(agent_name(brachio_agent::AgentKind::ClaudeCode)),
            agent_session_id: None,
            created_at_ms: now_ms(),
        };
        records.push(TaskRecord::insert(&transaction, &task).map_err(|error| error.to_string())?);
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(records)
}
