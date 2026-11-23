//! Takes over worktrees git already has for a project as tasks, without touching their files, and
//! lists the worktrees subagents made inside a task's folder.

use std::collections::HashMap;
use std::path::PathBuf;

use brachio_git::{importable, list_worktrees, select_importable, subagent_worktrees, Git};
use brachio_store::{NewTask, RepoRecord, TaskRecord};
use rusqlite::Connection;
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

/// Worktree folders of every task, open or archived.
fn task_paths(connection: &Connection) -> Result<Vec<PathBuf>, String> {
    Ok(TaskRecord::list_open(connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .chain(TaskRecord::list_archived(connection).map_err(|error| error.to_string())?)
        .map(|task| task.worktree_path)
        .collect())
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
    let mut known = task_paths(&connection)?;
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
            compare_group: None,
            created_at_ms: now_ms(),
        };
        records.push(TaskRecord::insert(&transaction, &task).map_err(|error| error.to_string())?);
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(records)
}

/// A worktree a subagent made inside a task's folder, under `.claude/worktrees/`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentWorktree {
    path: String,
    /// `None` for a detached HEAD.
    branch: Option<String>,
}

/// At most this many tasks are looked up in one call.
const MAX_SUBAGENT_TASKS: usize = 64;

/// The subagent worktrees inside each task's folder, by task id. Tasks without any are left out,
/// and so is a project git cannot list, so the sidebar shows nothing for it. Worktrees that are
/// already tasks are not listed.
#[tauri::command]
pub async fn task_subagent_worktrees(
    database: State<'_, Database>,
    ids: Vec<String>,
) -> Result<HashMap<String, Vec<SubagentWorktree>>, String> {
    let (projects, known) = {
        let connection = database.lock()?;
        let mut projects: HashMap<String, (PathBuf, Vec<TaskRecord>)> = HashMap::new();
        for id in ids.iter().take(MAX_SUBAGENT_TASKS) {
            let Ok(task) = TaskRecord::get(&connection, id) else {
                continue;
            };
            if let Some((_, tasks)) = projects.get_mut(&task.repo_id) {
                tasks.push(task);
                continue;
            }
            let Ok(repo) = RepoRecord::get(&connection, &task.repo_id) else {
                continue;
            };
            if repo.is_git {
                projects.insert(repo.id, (repo.path, vec![task]));
            }
        }
        (projects, task_paths(&connection)?)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let mut found: HashMap<String, Vec<SubagentWorktree>> = HashMap::new();
        for (repo_path, tasks) in projects.into_values() {
            let Ok(entries) = list_worktrees(&Git::new(&repo_path)) else {
                continue;
            };
            let folders: Vec<PathBuf> = tasks
                .iter()
                .map(|task| task.worktree_path.clone())
                .collect();
            let grouped = subagent_worktrees(&entries, &folders, &known);
            for (task, worktrees) in tasks.into_iter().zip(grouped) {
                if worktrees.is_empty() {
                    continue;
                }
                let listed = worktrees
                    .into_iter()
                    .map(|entry| SubagentWorktree {
                        path: entry.path.to_string_lossy().into_owned(),
                        branch: entry.branch,
                    })
                    .collect();
                found.insert(task.id, listed);
            }
        }
        found
    })
    .await
    .map_err(|error| error.to_string())
}
