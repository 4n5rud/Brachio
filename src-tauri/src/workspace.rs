//! IPC commands for repositories and tasks: add a repository, create a task
//! with its own worktree and branch, and list what is stored.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use brachio_agent::AgentKind;
use brachio_git::{predict_conflict, AddWorktree, Git, PullRequest};
use brachio_store::{ChildInsertError, NewRepo, NewTask, RepoRecord, TaskRecord};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::runtime_terminals::RuntimeTerminals;
use crate::terminal::Terminals;

/// App state that owns the database connection.
pub struct Database(pub Mutex<Connection>);

impl Database {
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.0
            .lock()
            .map_err(|_| "database lock poisoned".to_owned())
    }
}

#[derive(Default)]
pub struct TaskWorktreeLocks(Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>);

impl TaskWorktreeLocks {
    pub(crate) async fn acquire(
        &self,
        repo_id: &str,
    ) -> Result<tokio::sync::OwnedMutexGuard<()>, String> {
        let lock = {
            let mut locks = self
                .0
                .lock()
                .map_err(|_| "worktree lock poisoned".to_owned())?;
            locks.retain(|_, lock| lock.strong_count() != 0);
            if let Some(lock) = locks.get(repo_id).and_then(Weak::upgrade) {
                lock
            } else {
                let lock = Arc::new(tokio::sync::Mutex::new(()));
                locks.insert(repo_id.to_owned(), Arc::downgrade(&lock));
                lock
            }
        };
        Ok(lock.lock_owned().await)
    }
}

/// A new task from the "New task" dialog.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    /// Repository to branch from.
    pub repo_id: String,
    /// Existing parent task in this repository, if requested.
    pub parent_task_id: Option<String>,
    /// Task title. The branch name is derived from it.
    pub title: String,
    /// User's original task prompt.
    #[serde(default)]
    pub prompt: String,
    /// Agent to run in the task.
    pub agent: AgentKind,
    /// Commit-ish to branch from. Defaults to the repository's default branch.
    pub base: Option<String>,
    /// A Codex conversation recorded for the project folder that the new task continues.
    #[serde(default)]
    pub codex_conversation_id: Option<String>,
    /// Marks the task as one of several run on the same prompt to compare agents.
    #[serde(default)]
    pub compare_group: Option<String>,
    /// Set for tasks an agent asked for: the parent's open children are counted again when the
    /// task is saved, since other requests may have been approved meanwhile.
    #[serde(skip)]
    pub max_open_children: Option<i64>,
    /// The parent lead schedules its children, so the child waits for the scheduler to start it.
    #[serde(skip)]
    pub scheduled: bool,
    /// Ids of sibling tasks the child waits for, recorded when it is saved.
    #[serde(skip)]
    pub after: Vec<String>,
}

/// Lists registered repositories.
#[tauri::command]
pub fn repo_list(database: State<'_, Database>) -> Result<Vec<RepoRecord>, String> {
    RepoRecord::list(&*database.lock()?).map_err(to_text)
}

/// Adds the repository that contains `path`, or the folder itself when it is not in a git
/// repository: a folder project runs its tasks in the folder, with no branch or worktree. Adding
/// it twice returns the existing entry. With `git_only`, a folder that is not in a git repository
/// is refused instead of becoming a folder project.
#[tauri::command]
pub fn repo_add(
    database: State<'_, Database>,
    path: PathBuf,
    git_only: Option<bool>,
) -> Result<RepoRecord, String> {
    add_project(&database, &path, git_only.unwrap_or(false))
}

fn add_project(database: &Database, path: &Path, git_only: bool) -> Result<RepoRecord, String> {
    let (root, default_branch, is_git) = match brachio_git::inspect_repo(path) {
        Ok(info) => (info.root, info.default_branch, true),
        Err(brachio_git::GitError::NotARepository { .. }) if git_only => {
            return Err(format!(
                "not a git repository: {}. Add it as a folder workspace instead.",
                path.display()
            ));
        }
        Err(brachio_git::GitError::NotARepository { .. }) => {
            let folder = path
                .canonicalize()
                .map_err(|error| crate::folder_access::tag(path, error.to_string()))?;
            if !folder.is_dir() {
                return Err(format!("{} is not a folder.", path.display()));
            }
            (
                brachio_pty::without_verbatim_prefix(&folder),
                String::new(),
                false,
            )
        }
        Err(error) => return Err(crate::folder_access::tag(path, error.to_string())),
    };
    let connection = database.lock()?;
    if let Some(existing) = RepoRecord::find_by_path(&connection, &root).map_err(to_text)? {
        return Ok(existing);
    }
    let id = new_id();
    let repo = NewRepo {
        id: &id,
        name: &folder_name(&root),
        path: &root,
        default_branch: &default_branch,
        added_at_ms: now_ms(),
        is_git,
    };
    RepoRecord::insert(&connection, &repo).map_err(to_text)
}

/// Local and remote branches of a git project, for the base field of a new task. Folder projects
/// have none.
#[tauri::command]
pub async fn repo_branches(
    database: State<'_, Database>,
    id: String,
) -> Result<Vec<BranchChoice>, String> {
    let repo = RepoRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    if !repo.is_git {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(move || {
        brachio_git::list_branches(&Git::new(&repo.path))
            .map(|entries| {
                entries
                    .into_iter()
                    .map(|entry| BranchChoice {
                        name: entry.name,
                        value: entry.value,
                        local: entry.local,
                        remote: entry.remote,
                    })
                    .collect()
            })
            .map_err(to_text)
    })
    .await
    .map_err(to_text)?
}

/// One branch offered as the base of a new task.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchChoice {
    name: String,
    value: String,
    local: bool,
    remote: Option<String>,
}

/// A git repository found inside a folder project.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRepo {
    path: PathBuf,
    name: String,
    /// The repository is already a project of its own.
    added: bool,
}

/// The git repositories inside a folder project, so they can be added as projects of their own.
#[tauri::command]
pub async fn folder_source_repos(
    database: State<'_, Database>,
    id: String,
) -> Result<Vec<FolderRepo>, String> {
    let (folder, known) = {
        let connection = database.lock()?;
        let repo = RepoRecord::get(&connection, &id).map_err(to_text)?;
        if repo.is_git {
            return Ok(Vec::new());
        }
        let known: Vec<PathBuf> = RepoRecord::list(&connection)
            .map_err(to_text)?
            .into_iter()
            .map(|project| project.path)
            .collect();
        (repo.path, known)
    };
    tauri::async_runtime::spawn_blocking(move || {
        brachio_git::find_repositories(&folder)
            .into_iter()
            .map(|path| {
                let path = brachio_pty::without_verbatim_prefix(&path);
                FolderRepo {
                    name: folder_name(&path),
                    added: known.contains(&path),
                    path,
                }
            })
            .collect()
    })
    .await
    .map_err(to_text)
}

/// Whether the task belongs to a plain folder project, where no git feature applies.
fn is_folder_task(database: &Database, task: &TaskRecord) -> Result<bool, String> {
    let connection = database.lock()?;
    Ok(!RepoRecord::get(&connection, &task.repo_id)
        .map_err(to_text)?
        .is_git)
}

/// Takes the repository's worktree lock, then loads a task that is still open in a git project.
/// The task is read again after the wait because a deletion may have run in between.
async fn lock_open_git_task(
    database: &Database,
    worktrees: &TaskWorktreeLocks,
    id: &str,
) -> Result<(TaskRecord, tokio::sync::OwnedMutexGuard<()>), String> {
    let repo_id = TaskRecord::get(&*database.lock()?, id)
        .map_err(to_text)?
        .repo_id;
    let guard = worktrees.acquire(&repo_id).await?;
    let task = TaskRecord::get(&*database.lock()?, id).map_err(to_text)?;
    if task.archived_at_ms.is_some() {
        return Err("This task is archived. Restore it before running Git actions.".to_owned());
    }
    if is_folder_task(database, &task)? {
        return Err(NOT_GIT.to_owned());
    }
    Ok((task, guard))
}

const NOT_GIT: &str = "This project is a plain folder, so this needs git, which it does not use.";

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

/// A task that was just created.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedTask {
    #[serde(flatten)]
    pub(crate) task: TaskRecord,
    /// Commands from the repository's `brachio.toml` that the user is asked to run in the new
    /// worktree. Nothing runs without the user's say-so.
    setup_commands: Vec<String>,
    /// The file copy error that can be retried separately.
    setup_copy_error: Option<String>,
}

impl CreatedTask {
    /// A task the scheduler starts later, shaped like the one a person creates so the window
    /// opens it the same way. The files were copied when the task was created.
    pub(crate) fn started(task: TaskRecord, setup_commands: Vec<String>) -> Self {
        Self {
            task,
            setup_commands,
            setup_copy_error: None,
        }
    }
}

/// Creates a worktree on a fresh branch and stores the task. Files named in the repository's
/// `brachio.toml` are copied into the worktree, and its setup commands are returned.
///
/// git runs off the UI thread, since `worktree add` checks out files.
#[tauri::command]
pub async fn task_create(
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    request: CreateTaskRequest,
) -> Result<CreatedTask, String> {
    create_task(&database, &worktrees, request).await
}

/// The work of [`task_create`], shared with tasks that agents ask for.
pub(crate) async fn create_task(
    database: &Database,
    worktrees: &TaskWorktreeLocks,
    request: CreateTaskRequest,
) -> Result<CreatedTask, String> {
    let _guard = worktrees.acquire(&request.repo_id).await?;
    let (repo, parent_branch) = {
        let connection = database.lock()?;
        let repo = RepoRecord::get(&connection, &request.repo_id).map_err(to_text)?;
        let parent_branch = if let Some(parent_id) = request.parent_task_id.as_deref() {
            let parent = TaskRecord::get(&connection, parent_id).map_err(to_text)?;
            if parent.repo_id != repo.id || parent.archived_at_ms.is_some() {
                return Err("Choose an open parent task from this repository.".to_owned());
            }
            Some(parent.branch)
        } else {
            None
        };
        (repo, parent_branch)
    };
    check_project_conversation(&repo.path, &request).await?;
    if !repo.is_git {
        return create_folder_task(database, &repo, &request);
    }
    // Read first, so a broken file stops the task before anything is created.
    let setup = brachio_git::read_setup(&repo.path)
        .map_err(|error| crate::folder_access::tag(&repo.path, error.to_string()))?;
    let id = new_id();
    let branch = brachio_git::branch_name(&request.title, &id[..6]);
    let path = brachio_git::worktree_path(&repo.path, &branch);
    let add = AddWorktree {
        branch: branch.clone(),
        path: path.clone(),
        base: request
            .base
            .clone()
            .or(parent_branch)
            .unwrap_or_else(|| repo.default_branch.clone()),
    };
    let repo_path = repo.path.clone();
    let added = tauri::async_runtime::spawn_blocking(move || {
        brachio_git::add_worktree(&Git::new(&repo_path), &add)
    });
    added
        .await
        .map_err(to_text)?
        .map_err(|error| crate::folder_access::tag(&repo.path, error.to_string()))?;

    // Pinning the conversation id lets the task resume exactly its own
    // conversation after a restart.
    let session_id = task_session_id(&request);
    let task = NewTask {
        id: &id,
        repo_id: &repo.id,
        parent_task_id: request.parent_task_id.as_deref(),
        title: request.title.trim(),
        prompt: &request.prompt,
        branch: &branch,
        worktree_path: &path,
        agent_kind: Some(agent_name(request.agent)),
        agent_session_id: session_id.as_deref(),
        compare_group: request.compare_group.as_deref(),
        created_at_ms: now_ms(),
    };
    let mut refused = false;
    let saved = database.lock().and_then(|mut connection| {
        if let (Some(max), Some(_)) = (request.max_open_children, task.parent_task_id) {
            return TaskRecord::insert_child_after(&mut connection, &task, max, &request.after)
                .map_err(|error| {
                    refused = !matches!(error, ChildInsertError::Store(_));
                    match error {
                        ChildInsertError::LimitReached => {
                            format!(
                                "This task already has {max} open child tasks. Archive one first."
                            )
                        }
                        other => other.to_string(),
                    }
                });
        }
        let transaction = connection.transaction().map_err(to_text)?;
        let record = TaskRecord::insert(&transaction, &task).map_err(to_text)?;
        transaction.commit().map_err(to_text)?;
        Ok(record)
    });
    let task = match saved {
        Ok(task) => task,
        Err(error) => {
            let location = path.display().to_string();
            let cleanup = tauri::async_runtime::spawn_blocking(move || {
                brachio_git::remove_worktree(&Git::new(&repo.path), &path, &branch)
            })
            .await
            .map_err(to_text)
            .and_then(|result| result.map_err(to_text));
            return Err(match cleanup {
                Ok(()) if refused => error,
                Ok(()) => format!("Could not register task. The new worktree was removed. {error}"),
                Err(cleanup_error) => format!(
                    "Could not register task. Git could not finish cleanup at {location}: {cleanup_error}. {error}"
                ),
            });
        }
    };
    if task.parent_task_id.is_some() && !request.scheduled {
        let started = database.lock().and_then(|mut c| {
            brachio_store::mark_running(&mut c, &task.id, now_ms()).map_err(to_text)
        });
        if let Err(error) = started {
            tracing::warn!(%error, "could not record that the child task started");
        }
    }
    let setup_copy_error = brachio_git::copy_setup_files(&repo.path, &path, &setup)
        .err()
        .map(to_text);
    Ok(CreatedTask {
        task,
        setup_commands: setup.run,
        setup_copy_error,
    })
}

/// The conversation id a new task starts with: the Codex conversation the user picked from the
/// project, or a fresh id for agents that take one.
fn task_session_id(request: &CreateTaskRequest) -> Option<String> {
    request.codex_conversation_id.clone().or_else(|| {
        request
            .agent
            .accepts_session_id()
            .then(|| uuid::Uuid::new_v4().hyphenated().to_string())
    })
}

/// Refuses a picked Codex conversation unless the task is a Codex task and Codex recorded the
/// conversation for this project's own folder.
async fn check_project_conversation(
    project: &Path,
    request: &CreateTaskRequest,
) -> Result<(), String> {
    let Some(chosen) = request.codex_conversation_id.clone() else {
        return Ok(());
    };
    if request.agent != AgentKind::Codex {
        return Err("A Codex conversation can only start a Codex task.".to_owned());
    }
    let project = project.to_path_buf();
    let known = tauri::async_runtime::spawn_blocking(move || {
        project_conversation_exists(&brachio_agent::codex_conversations(&project), &chosen)
    })
    .await
    .map_err(to_text)?;
    if known {
        Ok(())
    } else {
        Err("The chosen Codex conversation is not recorded for this project.".to_owned())
    }
}

/// Codex ids are UUIDs; anything else never reaches a command line.
pub(crate) fn project_conversation_exists(
    listed: &[brachio_agent::CodexConversation],
    chosen: &str,
) -> bool {
    uuid::Uuid::parse_str(chosen).is_ok() && listed.iter().any(|item| item.id == chosen)
}

/// Codex conversations recorded for a project's own folder, offered before a task exists.
#[tauri::command]
pub async fn repo_codex_conversations(
    database: State<'_, Database>,
    id: String,
) -> Result<Vec<brachio_agent::CodexConversation>, String> {
    let repo = RepoRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        brachio_agent::codex_conversations(&repo.path)
            .into_iter()
            .filter(|item| uuid::Uuid::parse_str(&item.id).is_ok())
            .collect()
    })
    .await
    .map_err(to_text)
}

/// A task in a plain folder: it runs in the folder itself, so there is no branch to make and no
/// setup to copy.
fn create_folder_task(
    database: &Database,
    repo: &RepoRecord,
    request: &CreateTaskRequest,
) -> Result<CreatedTask, String> {
    let id = new_id();
    let session_id = task_session_id(request);
    let task = NewTask {
        id: &id,
        repo_id: &repo.id,
        parent_task_id: None,
        title: request.title.trim(),
        prompt: &request.prompt,
        branch: "",
        worktree_path: &repo.path,
        agent_kind: Some(agent_name(request.agent)),
        agent_session_id: session_id.as_deref(),
        compare_group: None,
        created_at_ms: now_ms(),
    };
    let record = TaskRecord::insert(&*database.lock()?, &task).map_err(to_text)?;
    Ok(CreatedTask {
        task: record,
        setup_commands: Vec::new(),
        setup_copy_error: None,
    })
}

/// Retries only missing setup files, preserving any files already present.
#[tauri::command]
pub async fn task_retry_setup_copy(
    database: State<'_, Database>,
    id: String,
) -> Result<(), String> {
    let (repo, task) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &id).map_err(to_text)?;
        if task.archived_at_ms.is_some() {
            return Err("Restore the task before retrying setup.".to_owned());
        }
        let repo = RepoRecord::get(&connection, &task.repo_id).map_err(to_text)?;
        (repo, task)
    };
    if !repo.is_git {
        return Ok(());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let setup = brachio_git::read_setup(&repo.path).map_err(to_text)?;
        brachio_git::retry_setup_files(&repo.path, &task.worktree_path, &setup)
            .map(|_| ())
            .map_err(to_text)
    })
    .await
    .map_err(to_text)?
}

/// Hides a task from the list. The worktree and branch stay on disk.
#[tauri::command]
pub fn task_archive(database: State<'_, Database>, id: String) -> Result<(), String> {
    archive_unless_children_open(&*database.lock()?, &id, now_ms())
}

/// Stable start of the refusal, so the UI can recognise it.
const OPEN_CHILDREN_PREFIX: &str = "has open children:";

/// Archives the task, but not a lead whose child tasks are still open: they could no longer reach
/// their parent with `send --to parent`, and nothing would show that they lost their supervisor.
fn archive_unless_children_open(
    connection: &Connection,
    id: &str,
    now_ms: i64,
) -> Result<(), String> {
    let children: Vec<String> = TaskRecord::list_open(connection)
        .map_err(to_text)?
        .into_iter()
        .filter(|task| task.parent_task_id.as_deref() == Some(id))
        .map(|task| task.title)
        .collect();
    if !children.is_empty() {
        return Err(format!(
            "{OPEN_CHILDREN_PREFIX} {}. Archive or finish these child tasks first, then archive the lead.",
            children.join(", ")
        ));
    }
    TaskRecord::archive(connection, id, now_ms).map_err(to_text)
}

/// Lists archived task records.
#[tauri::command]
pub fn task_list_archived(database: State<'_, Database>) -> Result<Vec<TaskRecord>, String> {
    TaskRecord::list_archived(&*database.lock()?).map_err(to_text)
}

/// Restores an archived task to the open list.
#[tauri::command]
pub fn task_restore(database: State<'_, Database>, id: String) -> Result<TaskRecord, String> {
    TaskRecord::restore(&*database.lock()?, &id).map_err(to_text)
}

/// Converts dropped files and folders into agent draft paths: worktree-relative inside the
/// worktree, absolute elsewhere.
#[tauri::command]
pub async fn task_drop_paths(
    database: State<'_, Database>,
    id: String,
    paths: Vec<PathBuf>,
) -> Result<Vec<String>, String> {
    if paths.is_empty() || paths.len() > 16 {
        return Err("Drop between 1 and 16 files.".to_owned());
    }
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = task.worktree_path.canonicalize().map_err(to_text)?;
        let mut files = Vec::new();
        for path in paths {
            let path = dropped_path(&root, &path)?;
            if !files.contains(&path) {
                files.push(path);
            }
        }
        Ok(files)
    })
    .await
    .map_err(to_text)?
}

fn dropped_path(root: &Path, path: &Path) -> Result<String, String> {
    if !path.is_absolute() {
        return Err("The dropped path is not absolute.".to_owned());
    }
    let file = path.canonicalize().map_err(to_text)?;
    let relative = file.strip_prefix(root).ok();
    if relative.is_some_and(|relative| {
        relative
            .components()
            .any(|component| component.as_os_str().eq_ignore_ascii_case(".git"))
    }) {
        return Err("Files inside .git cannot be dropped.".to_owned());
    }
    let text = match relative {
        Some(relative) => {
            let parts = relative
                .components()
                .map(|component| component.as_os_str().to_str())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| "The dropped path is not valid UTF-8.".to_owned())?;
            let text = parts.join("/");
            if cfg!(windows) && text.contains(':') {
                return Err("The dropped path contains an unsupported character.".to_owned());
            }
            text
        }
        None => strip_verbatim_prefix(
            file.to_str()
                .ok_or_else(|| "The dropped path is not valid UTF-8.".to_owned())?,
        ),
    };
    if text.chars().any(char::is_control) {
        return Err("The dropped path contains an unsupported character.".to_owned());
    }
    Ok(text)
}

/// Turns a Windows verbatim path into the plain form agents expect.
fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        path.to_owned()
    }
}

/// Lists the files of a task's worktree, tracked and not ignored, for quick open.
#[tauri::command]
pub async fn task_files(database: State<'_, Database>, id: String) -> Result<Vec<String>, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    let folder = is_folder_task(&database, &task)?;
    tauri::async_runtime::spawn_blocking(move || {
        if folder {
            return Ok(list_folder_files(&task.worktree_path));
        }
        brachio_git::list_files(&Git::new(&task.worktree_path)).map_err(to_text)
    })
    .await
    .map_err(to_text)?
}

/// The files of a plain folder, relative and with `/`, skipping folders that only hold
/// dependencies, build output or git data. Like a git listing, the first `MAX_FILES` in sorted
/// order are kept, so the cut does not depend on the order the filesystem lists entries in.
fn list_folder_files(root: &Path) -> Vec<String> {
    list_folder_files_capped(root, brachio_git::MAX_FILES)
}

fn list_folder_files_capped(root: &Path, limit: usize) -> Vec<String> {
    const SKIPPED: [&str; 4] = [".git", "node_modules", "target", ".venv"];
    // A max-heap of the smallest paths so far keeps memory at `limit` entries.
    let mut smallest = std::collections::BinaryHeap::with_capacity(limit.saturating_add(1));
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !SKIPPED.contains(&name.as_str()) {
                    pending.push(path);
                }
            } else if kind.is_file() {
                if let Ok(relative) = path.strip_prefix(root) {
                    smallest.push(relative.to_string_lossy().replace('\\', "/"));
                    if smallest.len() > limit {
                        smallest.pop();
                    }
                }
            }
        }
    }
    smallest.into_sorted_vec()
}

#[derive(Serialize)]
pub struct FilePreview {
    text: Option<String>,
    truncated: bool,
}

#[tauri::command]
pub async fn task_file_preview(
    database: State<'_, Database>,
    id: String,
    path: String,
) -> Result<FilePreview, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || read_file_preview(&task.worktree_path, &path))
        .await
        .map_err(to_text)?
}

/// Reads a supported media file without passing its contents through JSON.
#[tauri::command]
pub async fn task_file_media(
    database: State<'_, Database>,
    id: String,
    path: String,
) -> Result<tauri::ipc::Response, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        const LIMIT: usize = 8 * 1024 * 1024;
        let file = resolve_preview_path(&task.worktree_path, &path)?;
        let bytes = read_preview_bytes(&file, LIMIT)?;
        if bytes.len() > LIMIT {
            return Err("Media preview is limited to 8 MiB.".to_owned());
        }
        let valid = match Path::new(&path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            Some("jpg" | "jpeg") => bytes.starts_with(b"\xff\xd8\xff"),
            Some("gif") => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
            Some("webp") => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            Some("pdf") => bytes.starts_with(b"%PDF-"),
            _ => false,
        };
        if !valid {
            return Err("The file does not match a supported media format.".to_owned());
        }
        Ok(tauri::ipc::Response::new(bytes))
    })
    .await
    .map_err(to_text)?
}

fn resolve_preview_path(worktree: &Path, path: &str) -> Result<PathBuf, String> {
    use std::path::Component;

    let relative = Path::new(path);
    if path.is_empty()
        || (cfg!(windows) && path.contains(':'))
        || relative.components().any(|component| match component {
            Component::Normal(name) => name.eq_ignore_ascii_case(".git"),
            _ => true,
        })
    {
        return Err("Choose a file inside the task's worktree.".to_owned());
    }
    let root = worktree.canonicalize().map_err(to_text)?;
    let file = root.join(relative).canonicalize().map_err(to_text)?;
    let resolved = file.strip_prefix(&root).map_err(to_text)?;
    if !file.is_file()
        || resolved
            .components()
            .any(|component| component.as_os_str().eq_ignore_ascii_case(".git"))
    {
        return Err("Choose a regular file inside the task's worktree.".to_owned());
    }
    Ok(file)
}

fn read_preview_bytes(file: &Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .map_err(to_text)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(to_text)?;
    Ok(bytes)
}

fn read_file_preview(worktree: &Path, path: &str) -> Result<FilePreview, String> {
    const LIMIT: usize = 64 * 1024;
    let file = resolve_preview_path(worktree, path)?;
    let mut bytes = read_preview_bytes(&file, LIMIT)?;
    let truncated = bytes.len() > LIMIT;
    bytes.truncate(LIMIT);
    let text = if bytes.contains(&0) {
        None
    } else {
        match std::str::from_utf8(&bytes) {
            Ok(text) => Some(text.to_owned()),
            Err(error) if truncated && error.error_len().is_none() => {
                std::str::from_utf8(&bytes[..error.valid_up_to()])
                    .ok()
                    .map(str::to_owned)
            }
            Err(_) => None,
        }
    };
    Ok(FilePreview { text, truncated })
}

/// What deleting an archived task would destroy.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLoss {
    uncommitted_files: u32,
    unpushed_commits: u32,
    ignored_entries: u32,
    ignored_examples: Vec<String>,
    /// Changes whenever anything the user was shown, or the branch head, changes.
    fingerprint: String,
    running_terminals: usize,
    /// The liveness check failed, so `running_terminals` says nothing.
    terminals_unknown: bool,
}

/// Refuses deletion when the loss is no longer what the user confirmed.
fn ensure_loss_unchanged(confirmed: &str, current: &str) -> Result<(), String> {
    if confirmed == current {
        return Ok(());
    }
    Err(
        "task changed: The task changed after you reviewed it. Review the deletion again."
            .to_owned(),
    )
}

/// Stable start of the refusal, so the UI can recognise it.
const TERMINAL_RUNNING_PREFIX: &str = "terminal still running:";

/// Refuses deletion while a terminal of the task runs, or when that could not be checked.
fn ensure_no_live_terminals(live: Result<Vec<String>, String>) -> Result<(), String> {
    let keys =
        live.map_err(|_| "could not verify that the task's terminals are stopped".to_owned())?;
    if keys.is_empty() {
        return Ok(());
    }
    let panes: Vec<&str> = keys
        .iter()
        .map(|key| key.split_once(':').map_or(key.as_str(), |(_, pane)| pane))
        .collect();
    Err(format!(
        "{TERMINAL_RUNNING_PREFIX} This task still has a running terminal ({}). Stop it, then delete the task.",
        panes.join(", ")
    ))
}

/// Terminals of the task that are still running, in the runtime or in the app, or why that could
/// not be told. Tabs the task has since closed are asked for too.
pub(crate) async fn live_task_terminals(
    database: &Database,
    terminals: &RuntimeTerminals,
    in_app: &Terminals,
    task_id: &str,
) -> Result<Vec<String>, String> {
    let (current, retired) = {
        let connection = database.lock()?;
        let current: Vec<String> = brachio_store::task_tabs(&connection, task_id)
            .map_err(to_text)?
            .into_iter()
            .map(|tab| tab.tab_id)
            .collect();
        let retired =
            brachio_store::retired_secondary_tabs(&connection, task_id).map_err(to_text)?;
        (current, retired)
    };
    let tabs: Vec<String> = current.into_iter().chain(retired.iter().cloned()).collect();
    let mut live = terminals
        .live_keys_for_task(task_id, &tabs)
        .await
        .map_err(to_text)?;
    live.extend(
        in_app
            .1
            .live_for_task(&in_app.0, task_id)
            .map_err(to_text)?,
    );
    let connection = database.lock()?;
    for tab in retired {
        if !live.contains(&format!("{task_id}:agent-{tab}"))
            && !live.contains(&format!("{task_id}:shell-{tab}"))
        {
            // A failed cleanup only keeps the id for the next check.
            let _ = brachio_store::forget_retired_secondary_tab(&connection, task_id, &tab);
        }
    }
    Ok(live)
}

/// Loads an archived task and the repository it belongs to. Only archived tasks can be deleted,
/// so no terminal is still using the worktree.
fn archived_task(database: &Database, id: &str) -> Result<(TaskRecord, RepoRecord), String> {
    let connection = database.lock()?;
    let task = TaskRecord::get(&connection, id).map_err(to_text)?;
    if task.archived_at_ms.is_none() {
        return Err("Archive the task before deleting it.".to_owned());
    }
    if TaskRecord::has_children(&connection, id).map_err(to_text)? {
        return Err("Delete the child tasks before deleting their parent.".to_owned());
    }
    let repo = RepoRecord::get(&connection, &task.repo_id).map_err(to_text)?;
    Ok((task, repo))
}

/// Counts the work that deleting an archived task would lose, so the user can confirm it.
#[tauri::command]
pub async fn task_delete_preview(
    database: State<'_, Database>,
    terminals: State<'_, RuntimeTerminals>,
    in_app: State<'_, Terminals>,
    id: String,
) -> Result<TaskLoss, String> {
    let (task, repo) = archived_task(&database, &id)?;
    let (running_terminals, terminals_unknown) =
        match live_task_terminals(&database, &terminals, &in_app, &id).await {
            Ok(keys) => (keys.len(), false),
            Err(error) => {
                tracing::warn!(%error, "could not check the task's terminals");
                (0, true)
            }
        };
    if !repo.is_git {
        return Ok(TaskLoss {
            uncommitted_files: 0,
            unpushed_commits: 0,
            ignored_entries: 0,
            ignored_examples: Vec::new(),
            fingerprint: String::new(),
            running_terminals,
            terminals_unknown,
        });
    }
    let work = tauri::async_runtime::spawn_blocking(move || {
        brachio_git::unsaved_work(&Git::new(&repo.path), &task.worktree_path, &task.branch)
    })
    .await
    .map_err(to_text)?
    .map_err(to_text)?;
    Ok(TaskLoss {
        uncommitted_files: work.uncommitted_files,
        unpushed_commits: work.unpushed_commits,
        ignored_entries: work.ignored_entries,
        ignored_examples: work.ignored_examples,
        fingerprint: work.fingerprint,
        running_terminals,
        terminals_unknown,
    })
}

/// Deletes an archived task for good: its worktree folder, its branch and its record. The caller
/// has shown `task_delete_preview` to the user.
#[tauri::command]
pub async fn task_delete(
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    terminals: State<'_, RuntimeTerminals>,
    in_app: State<'_, Terminals>,
    id: String,
    expected_fingerprint: String,
) -> Result<(), String> {
    delete_task(
        &database,
        &worktrees,
        &terminals,
        &in_app,
        &id,
        &expected_fingerprint,
    )
    .await
}

async fn delete_task(
    database: &Database,
    worktrees: &TaskWorktreeLocks,
    terminals: &RuntimeTerminals,
    in_app: &Terminals,
    id: &str,
    expected_fingerprint: &str,
) -> Result<(), String> {
    let repo_id = TaskRecord::get(&*database.lock()?, id)
        .map_err(to_text)?
        .repo_id;
    let _guard = worktrees.acquire(&repo_id).await?;
    let (task, repo) = archived_task(database, id)?;
    ensure_no_live_terminals(live_task_terminals(database, terminals, in_app, id).await)?;
    // A folder project's task runs in the user's own folder, which must never be removed.
    if repo.is_git {
        let expected_fingerprint = expected_fingerprint.to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            let git = Git::new(&repo.path);
            let current = brachio_git::unsaved_work(&git, &task.worktree_path, &task.branch)
                .map_err(to_text)?;
            ensure_loss_unchanged(&expected_fingerprint, &current.fingerprint)?;
            brachio_git::discard_worktree(&git, &task.worktree_path, &task.branch).map_err(
                |error| {
                    let text = to_text(error);
                    crate::file_in_use::describe(&text).unwrap_or(text)
                },
            )
        })
        .await
        .map_err(to_text)??;
    }
    {
        let connection = database.lock()?;
        // Its messages go with it, so a later task cannot be sent mail meant for this one.
        brachio_store::forget_task_messages(&connection, id).map_err(to_text)?;
        TaskRecord::delete(&connection, id).map_err(to_text)?;
    }
    crate::runtime_terminals::forget_task_screens(id).await;
    Ok(())
}

/// Changes in a task worktree relative to its base.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskChanges {
    patch: String,
    untracked: Vec<String>,
    staged: Vec<String>,
    conflicts: Vec<String>,
    /// "rebase" or "merge" while git waits in the worktree, else null.
    operation: Option<&'static str>,
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
    if !repo.is_git {
        return Ok(TaskChanges {
            patch: String::new(),
            untracked: Vec::new(),
            staged: Vec::new(),
            conflicts: Vec::new(),
            operation: None,
            truncated: false,
        });
    }
    tauri::async_runtime::spawn_blocking(move || {
        const PATCH_LIMIT: usize = 256 * 1024;
        let git = Git::new(&task.worktree_path);
        let base_ref = format!("refs/heads/{}", repo.default_branch);
        let base = git
            .run(&["merge-base", "HEAD", &base_ref])
            .map_err(to_text)?;
        let (mut patch, truncated) = git
            .run_text_capped(
                &[
                    "-c",
                    "core.quotePath=false",
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "--src-prefix=a/",
                    "--dst-prefix=b/",
                    "--submodule=short",
                    &base,
                    "--",
                ],
                PATCH_LIMIT,
            )
            .map_err(to_text)?;
        if truncated {
            // Show whole lines only, so the last hunk is not broken mid-line.
            if let Some(end) = patch.rfind('\n') {
                patch.truncate(end + 1);
            }
        }
        let untracked = git_paths(&git, &["ls-files", "--others", "--exclude-standard", "-z"])?;
        let index = git
            .run_text(&[
                "diff",
                "--cached",
                "--no-renames",
                "--name-status",
                "-z",
                "--",
            ])
            .map_err(to_text)?;
        let (staged, conflicts) = index_paths(&index)?;
        Ok(TaskChanges {
            patch,
            untracked,
            staged,
            conflicts,
            operation: brachio_git::stopped_operation(&git)
                .map(brachio_git::StoppedOperation::as_str),
            truncated,
        })
    })
    .await
    .map_err(to_text)?
}

fn index_paths(output: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut staged = Vec::new();
    let mut conflicts = Vec::new();
    let mut entries = output.split_terminator('\0');
    while let Some(status) = entries.next() {
        let path = entries
            .next()
            .ok_or("Git returned an incomplete index diff.")?;
        if status == "U" {
            conflicts.push(path.to_owned());
        }
        staged.push(path.to_owned());
    }
    Ok((staged, conflicts))
}

#[cfg(test)]
mod folder_project_tests {
    use std::sync::Mutex;

    use brachio_agent::AgentKind;

    use super::{add_project, create_folder_task, list_folder_files, CreateTaskRequest, Database};

    fn database() -> Database {
        Database(Mutex::new(
            brachio_store::open_in_memory().expect("open database"),
        ))
    }

    #[test]
    fn a_folder_listing_keeps_the_first_files_in_sorted_order_whatever_the_walk_order() {
        let folder = tempfile::tempdir().expect("temp dir");
        for name in ["d.txt", "a.txt", "c.txt", "b.txt", "e.txt"] {
            std::fs::write(folder.path().join(name), "").expect("write");
        }
        std::fs::create_dir(folder.path().join("sub")).expect("dir");
        std::fs::write(folder.path().join("sub/a.txt"), "").expect("write");
        let files = super::list_folder_files_capped(folder.path(), 4);
        assert_eq!(files, ["a.txt", "b.txt", "c.txt", "d.txt"]);
        let all = super::list_folder_files_capped(folder.path(), 100);
        assert_eq!(all.len(), 6);
        assert_eq!(all[5], "sub/a.txt");
    }

    #[test]
    fn git_only_refuses_a_plain_folder_and_adds_nothing() {
        let folder = tempfile::tempdir().expect("temp dir");
        let database = database();
        let error = add_project(&database, folder.path(), true).expect_err("not a repository");
        assert!(error.starts_with("not a git repository: "), "{error}");
        assert!(
            error.ends_with("Add it as a folder workspace instead."),
            "{error}"
        );
        let listed =
            brachio_store::RepoRecord::list(&database.lock().expect("lock")).expect("list");
        assert!(listed.is_empty());
    }

    #[test]
    fn a_folder_that_is_not_a_repository_becomes_a_folder_project_once() {
        let folder = tempfile::tempdir().expect("temp dir");
        let database = database();
        let project = add_project(&database, folder.path(), false).expect("add the folder");
        assert!(!project.is_git);
        assert!(project.default_branch.is_empty());
        assert_eq!(
            brachio_pty::without_verbatim_prefix(&folder.path().canonicalize().expect("path")),
            project.path
        );
        let again = add_project(&database, folder.path(), false).expect("add it again");
        assert_eq!(again.id, project.id);
        assert!(add_project(&database, &folder.path().join("missing"), false).is_err());
    }

    #[test]
    fn a_task_in_a_folder_project_runs_in_the_folder_without_a_branch() {
        let folder = tempfile::tempdir().expect("temp dir");
        let database = database();
        let project = add_project(&database, folder.path(), false).expect("add the folder");
        let request = CreateTaskRequest {
            repo_id: project.id.clone(),
            parent_task_id: None,
            title: "Notes".to_owned(),
            prompt: String::new(),
            agent: AgentKind::ClaudeCode,
            base: Some("main".to_owned()),
            codex_conversation_id: None,
            compare_group: None,
            max_open_children: None,
            scheduled: false,
            after: Vec::new(),
        };
        let created = create_folder_task(&database, &project, &request).expect("create task");
        assert_eq!(created.task.worktree_path, project.path);
        assert!(created.task.branch.is_empty());
        assert!(created.setup_commands.is_empty());
        let other = create_folder_task(&database, &project, &request).expect("a second task");
        assert_ne!(
            other.task.id, created.task.id,
            "several tasks may share the folder"
        );
    }

    #[test]
    fn listing_a_folder_skips_dependency_and_git_folders_and_uses_forward_slashes() {
        let folder = tempfile::tempdir().expect("temp dir");
        for path in [
            "a.txt",
            "src/b.rs",
            "node_modules/x.js",
            ".git/config",
            "target/out.bin",
        ] {
            let file = folder.path().join(path);
            std::fs::create_dir_all(file.parent().expect("parent")).expect("make folder");
            std::fs::write(file, "x").expect("write file");
        }
        assert_eq!(list_folder_files(folder.path()), ["a.txt", "src/b.rs"]);
    }
}

#[cfg(test)]
mod deletion_loss_tests {
    use super::ensure_loss_unchanged;

    #[test]
    fn a_changed_fingerprint_refuses_with_the_stable_prefix() {
        assert_eq!(ensure_loss_unchanged("abc", "abc"), Ok(()));
        let refusal = ensure_loss_unchanged("abc", "abd").expect_err("must refuse");
        assert!(refusal.starts_with("task changed:"), "{refusal}");
    }
}

#[cfg(test)]
mod index_tests {
    use super::index_paths;

    #[test]
    fn index_diff_lists_conflicts_with_the_staged_paths() {
        let (staged, conflicts) =
            index_paths("A\0New file.txt\0U\0conflict.txt\0D\0old.txt\0").expect("index diff");
        assert_eq!(staged, ["New file.txt", "conflict.txt", "old.txt"]);
        assert_eq!(conflicts, ["conflict.txt"]);
        assert!(index_paths("U\0").is_err());
    }
}

fn git_paths(git: &Git, args: &[&str]) -> Result<Vec<String>, String> {
    Ok(git
        .run_text(args)
        .map_err(to_text)?
        .split_terminator('\0')
        .map(str::to_owned)
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMergePreview {
    base_branch: String,
    conflicts: Vec<String>,
    /// False when the comparison used the local branch because no remote copy exists.
    remote_tracking: bool,
}

/// Checks how the task's committed HEAD would merge into the default branch, as the remote has it.
#[tauri::command]
pub async fn task_merge_preview(
    database: State<'_, Database>,
    id: String,
) -> Result<TaskMergePreview, String> {
    let (worktree_path, base_branch, is_git) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &id).map_err(to_text)?;
        let repo = RepoRecord::get(&connection, &task.repo_id).map_err(to_text)?;
        (task.worktree_path, repo.default_branch, repo.is_git)
    };
    if !is_git {
        return Ok(TaskMergePreview {
            base_branch,
            conflicts: Vec::new(),
            remote_tracking: true,
        });
    }
    tauri::async_runtime::spawn_blocking(move || {
        let git = Git::new(&worktree_path);
        let target = brachio_git::merge_target(&git, &base_branch);
        let report = predict_conflict(&git, &target.reference, "HEAD").map_err(to_text)?;
        Ok(TaskMergePreview {
            base_branch,
            conflicts: report.conflicted_files,
            remote_tracking: target.remote_tracking,
        })
    })
    .await
    .map_err(to_text)?
}

/// A Git operation explicitly requested from the task's Changes panel.
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum TaskGitAction {
    /// Stage modifications and deletions to tracked files.
    StageTracked,
    /// Stage a single new file.
    StageFile {
        /// A literal path relative to the worktree.
        path: String,
    },
    /// Remove a file from the index while keeping its working copy.
    UnstageFile {
        /// A literal path relative to the worktree.
        path: String,
    },
    /// Commit the current index with the given message.
    Commit {
        /// User-entered commit message.
        message: String,
    },
    /// Push the task branch to its push remote, setting it as the upstream.
    Push,
    /// Open a pull request for the pushed task branch with the GitHub CLI.
    CreatePr {
        /// One-line title.
        title: String,
        /// Description; may be empty.
        body: String,
    },
}

/// Runs a Git action in the task worktree.
#[tauri::command]
pub async fn task_git_action(
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    id: String,
    request: TaskGitAction,
) -> Result<Option<String>, String> {
    // Held until the action ends so a deletion cannot remove the worktree underneath it.
    let (task, _guard) = lock_open_git_task(&database, &worktrees, &id).await?;
    let recorded = match &request {
        TaskGitAction::Commit { .. } => Some("commit"),
        TaskGitAction::Push => Some("push"),
        TaskGitAction::CreatePr { .. } => Some("pull request"),
        _ => None,
    };
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let git = Git::new(&task.worktree_path);
        let branch_ref = format!("refs/heads/{}", task.branch);
        let current_ref = git.run(&["symbolic-ref", "HEAD"]).map_err(to_text)?;
        if current_ref != branch_ref {
            return Err(format!("Expected {branch_ref}, found {current_ref}"));
        }
        if let TaskGitAction::CreatePr { title, body } = &request {
            return create_pull_request(&task, title, body).map(Some);
        }
        match request {
            TaskGitAction::StageTracked => git.run(&["add", "--update", "--"]),
            TaskGitAction::StageFile { path } => {
                git.run(&["--literal-pathspecs", "add", "--", &path])
            }
            TaskGitAction::UnstageFile { path } => {
                git.run(&["--literal-pathspecs", "restore", "--staged", "--", &path])
            }
            TaskGitAction::Commit { message } => {
                let message = message.trim();
                if message.is_empty() {
                    return Err("Enter a commit message.".to_owned());
                }
                git.run(&["commit", "-m", message])
            }
            TaskGitAction::Push => {
                let refspec = format!("{branch_ref}:{branch_ref}");
                brachio_git::push_remote(&git, &task.branch).and_then(|remote| {
                    git.run(&["push", "--set-upstream", "--", &remote, &refspec])
                })
            }
            TaskGitAction::CreatePr { .. } => unreachable!("handled before the match"),
        }
        .map_err(to_text)?;
        Ok(None)
    })
    .await
    .map_err(to_text)??;
    if let Some(label) = recorded {
        if let Err(error) = brachio_store::record_activity(
            &*database.lock()?,
            &id,
            "git_action",
            Some(label),
            now_ms(),
        ) {
            tracing::warn!(%error, "could not record the git action");
        }
    }
    Ok(outcome)
}

/// Opens a pull request from the task branch into the repository's default branch.
fn create_pull_request(task: &TaskRecord, title: &str, body: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Enter a pull request title.".to_owned());
    }
    let base = brachio_git::inspect_repo(&task.worktree_path)
        .map_err(to_text)?
        .default_branch;
    let request = PullRequest {
        base: &base,
        head: &task.branch,
        title,
        body: body.trim(),
    };
    brachio_git::create_pull_request(&task.worktree_path, &request).map_err(to_text)
}

/// Reads the PR associated with the task branch using the user's GitHub CLI login.
#[tauri::command]
pub async fn task_pr_feedback(
    database: State<'_, Database>,
    id: String,
) -> Result<serde_json::Value, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    if is_folder_task(&database, &task)? {
        return Err(NOT_GIT.to_owned());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let stdout = run_github(
            github_command()
                .current_dir(&task.worktree_path)
                .args([
                    "pr",
                    "view",
                    "--json",
                    "url,statusCheckRollup,reviews,comments",
                    "--",
                ])
                .arg(&task.branch),
            PR_VIEW_MAX_BYTES,
            "the pull request",
        )?;
        let mut feedback: serde_json::Value = serde_json::from_slice(&stdout).map_err(to_text)?;
        let (inline_comments, inline_error) =
            match read_inline_reviews(&task.worktree_path, &feedback) {
                Ok(comments) => (comments, None),
                Err(error) => (Vec::new(), Some(error)),
            };
        feedback["inlineComments"] = serde_json::json!(inline_comments);
        feedback["inlineError"] = serde_json::json!(inline_error);
        Ok(feedback)
    })
    .await
    .map_err(to_text)?
}

const GH_TIME_LIMIT: Duration = Duration::from_secs(60);
const PR_VIEW_MAX_BYTES: usize = 8 * 1024 * 1024;
const INLINE_REVIEWS_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Runs `gh` with a deadline and a stdout cap; an over-large or slow answer is an error, never a cut-off JSON.
fn run_github(
    command: &mut std::process::Command,
    max_stdout: usize,
    what: &str,
) -> Result<Vec<u8>, String> {
    github_output(
        brachio_process::output_capped(command, GH_TIME_LIMIT, max_stdout),
        max_stdout,
        what,
    )
}

fn github_output(
    result: std::io::Result<brachio_process::Capped>,
    max_stdout: usize,
    what: &str,
) -> Result<Vec<u8>, String> {
    let capped = result.map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => {
            "The GitHub CLI (gh) is not installed or not on PATH.".to_owned()
        }
        std::io::ErrorKind::TimedOut => format!("Loading {what} from GitHub {error}."),
        _ => error.to_string(),
    })?;
    if capped.truncated {
        return Err(format!(
            "{what} is larger than {} MiB, so it was not loaded.",
            max_stdout / (1024 * 1024)
        ));
    }
    if !capped.output.status.success() {
        return Err(String::from_utf8_lossy(&capped.output.stderr)
            .trim()
            .to_owned());
    }
    Ok(capped.output.stdout)
}

fn github_command() -> std::process::Command {
    let mut command = std::process::Command::new("gh");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.stdin(std::process::Stdio::null());
    command
}

fn read_inline_reviews(
    worktree: &Path,
    feedback: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, String> {
    let url = feedback["url"].as_str().unwrap_or_default();
    let parts: Vec<_> = url
        .strip_prefix("https://")
        .unwrap_or_default()
        .split('/')
        .collect();
    let [host, owner, repo, "pull", number] = parts.as_slice() else {
        return Err("Could not identify the repository for inline reviews.".to_owned());
    };
    let endpoint = format!("repos/{owner}/{repo}/pulls/{number}/comments");
    let stdout = run_github(
        github_command().current_dir(worktree).args([
            "api",
            "--hostname",
            host,
            "--paginate",
            "--slurp",
            &endpoint,
        ]),
        INLINE_REVIEWS_MAX_BYTES,
        "the inline review comments",
    )?;
    let pages: Vec<Vec<serde_json::Value>> = serde_json::from_slice(&stdout).map_err(to_text)?;
    Ok(pages
        .into_iter()
        .flatten()
        .map(|comment| {
            serde_json::json!({
                "body": comment["body"],
                "url": comment["html_url"],
                "author": comment["user"],
                "path": comment["path"],
                "line": comment["line"].as_u64().or_else(|| comment["original_line"].as_u64()),
            })
        })
        .collect())
}

/// Errors cross the IPC boundary as display strings.
fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub(crate) fn agent_name(agent: AgentKind) -> &'static str {
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

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub(crate) fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod agent_child_limit_tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::Mutex;

    use brachio_agent::AgentKind;
    use brachio_store::{open_in_memory, NewRepo, RepoRecord, TaskRecord};

    use super::{create_task, CreateTaskRequest, Database, TaskWorktreeLocks};

    fn git(folder: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(folder)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?}");
    }

    fn git_output(folder: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(folder)
            .args(args)
            .output()
            .expect("run git");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn request(title: &str) -> CreateTaskRequest {
        CreateTaskRequest {
            repo_id: "r".to_owned(),
            parent_task_id: Some("lead".to_owned()),
            title: title.to_owned(),
            prompt: String::new(),
            agent: AgentKind::ClaudeCode,
            base: None,
            codex_conversation_id: None,
            compare_group: None,
            max_open_children: Some(4),
            scheduled: false,
            after: Vec::new(),
        }
    }

    #[test]
    fn two_approved_requests_with_three_open_children_leave_four_and_no_extra_worktree() {
        let root = tempfile::tempdir().expect("temp dir");
        let repo_path = root.path().join("repo");
        std::fs::create_dir(&repo_path).expect("make repo");
        git(&repo_path, &["init", "-b", "main"]);
        git(&repo_path, &["commit", "--allow-empty", "-m", "start"]);
        let connection = open_in_memory().expect("open database");
        let repo = NewRepo {
            id: "r",
            name: "repo",
            path: &repo_path,
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(&connection, &repo).expect("insert repo");
        connection
            .execute_batch(
                "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('lead', 'r', 'Lead', '', 'main', '/w0', 'active', 0);
                 INSERT INTO tasks (id, repo_id, parent_task_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('c1', 'r', 'lead', 'C', '', 'b1', '/w1', 'active', 0),
                        ('c2', 'r', 'lead', 'C', '', 'b2', '/w2', 'active', 0),
                        ('c3', 'r', 'lead', 'C', '', 'b3', '/w3', 'active', 0);",
            )
            .expect("insert tasks");
        let database = Database(Mutex::new(connection));
        let locks = TaskWorktreeLocks::default();

        let (first, second) = tauri::async_runtime::block_on(async {
            tokio::join!(
                create_task(&database, &locks, request("First")),
                create_task(&database, &locks, request("Second"))
            )
        });

        let mut results = [first, second];
        results.sort_by_key(Result::is_err);
        assert!(
            results[0].is_ok(),
            "one request fits: {:?}",
            results[0].as_ref().err()
        );
        assert_eq!(
            results[1].as_ref().err().map(String::as_str),
            Some("This task already has 4 open child tasks. Archive one first.")
        );
        let connection = database.lock().expect("lock");
        let open: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE parent_task_id = 'lead' AND archived_at_ms IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(open, 4);
        assert_eq!(TaskRecord::count_for_repo(&connection, "r").expect("n"), 5);
        let listed = git_output(&repo_path, &["worktree", "list", "--porcelain"]);
        assert_eq!(listed.matches("worktree ").count(), 2, "{listed}");
        let branches = git_output(&repo_path, &["branch", "--format=%(refname:short)"]);
        assert_eq!(branches.lines().count(), 2, "{branches}");
    }
}

#[cfg(test)]
mod terminal_guard_tests {
    use super::ensure_no_live_terminals;

    #[test]
    fn no_live_terminal_allows_deletion() {
        assert_eq!(ensure_no_live_terminals(Ok(Vec::new())), Ok(()));
    }

    #[test]
    fn a_live_terminal_refuses_with_the_stable_prefix() {
        let refusal =
            ensure_no_live_terminals(Ok(vec!["t1:agent".to_owned()])).expect_err("refused");
        assert_eq!(
            refusal,
            "terminal still running: This task still has a running terminal (agent). Stop it, then delete the task."
        );
    }

    #[test]
    fn an_unanswered_runtime_refuses() {
        let refusal = ensure_no_live_terminals(Err("boom".to_owned())).expect_err("refused");
        assert_eq!(
            refusal,
            "could not verify that the task's terminals are stopped"
        );
    }
}

#[cfg(test)]
mod git_action_guard_tests {
    use std::future::Future;
    use std::path::Path;
    use std::pin::pin;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    use brachio_store::{open_in_memory, NewRepo, RepoRecord};

    use super::{lock_open_git_task, Database, TaskWorktreeLocks};

    fn database() -> Database {
        let connection = open_in_memory().expect("open database");
        for id in ["r", "s"] {
            let repo = NewRepo {
                id,
                name: id,
                path: Path::new(if id == "r" { "/repo-r" } else { "/repo-s" }),
                default_branch: "main",
                added_at_ms: 0,
                is_git: true,
            };
            RepoRecord::insert(&connection, &repo).expect("insert repo");
        }
        connection
            .execute_batch(
                "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, archived_at_ms)
                 VALUES ('open', 'r', 'T', '', 'b1', '/w1', 'active', 0, NULL),
                        ('old', 'r', 'T', '', 'b2', '/w2', 'active', 0, 1),
                        ('other', 's', 'T', '', 'b3', '/w3', 'active', 0, NULL);",
            )
            .expect("insert tasks");
        Database(Mutex::new(connection))
    }

    #[test]
    fn an_archived_task_is_refused_before_any_git_runs() {
        let database = database();
        let locks = TaskWorktreeLocks::default();
        let refusal = tauri::async_runtime::block_on(lock_open_git_task(&database, &locks, "old"))
            .map(|_| ())
            .expect_err("archived");
        assert_eq!(
            refusal,
            "This task is archived. Restore it before running Git actions."
        );
        let opened = tauri::async_runtime::block_on(lock_open_git_task(&database, &locks, "open"));
        assert!(opened.is_ok());
    }

    #[test]
    fn a_git_action_waits_for_a_deletion_in_the_same_repository_only() {
        let database = database();
        let locks = TaskWorktreeLocks::default();
        let deletion = tauri::async_runtime::block_on(locks.acquire("r")).expect("lock");
        let mut context = Context::from_waker(Waker::noop());

        let mut same = pin!(lock_open_git_task(&database, &locks, "open"));
        assert!(same.as_mut().poll(&mut context).is_pending());
        let mut independent = pin!(lock_open_git_task(&database, &locks, "other"));
        assert!(matches!(
            independent.as_mut().poll(&mut context),
            Poll::Ready(Ok(_))
        ));

        drop(deletion);
        assert!(matches!(
            same.as_mut().poll(&mut context),
            Poll::Ready(Ok(_))
        ));
    }
}

#[cfg(test)]
mod archive_children_tests {
    use std::path::Path;

    use brachio_store::{open_in_memory, NewRepo, NewTask, RepoRecord, TaskRecord};

    use super::archive_unless_children_open;

    fn task<'a>(id: &'a str, parent: Option<&'a str>) -> NewTask<'a> {
        NewTask {
            id,
            repo_id: "r",
            parent_task_id: parent,
            title: id,
            prompt: "",
            branch: id,
            worktree_path: Path::new("/w"),
            agent_kind: None,
            agent_session_id: None,
            compare_group: None,
            created_at_ms: 0,
        }
    }

    #[test]
    fn a_lead_with_an_open_child_is_refused_until_the_child_is_archived() {
        let connection = open_in_memory().expect("open");
        let repo = NewRepo {
            id: "r",
            name: "r",
            path: Path::new("/r"),
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(&connection, &repo).expect("repo");
        TaskRecord::insert(&connection, &task("lead", None)).expect("lead");
        TaskRecord::insert(&connection, &task("c1", Some("lead"))).expect("c1");
        TaskRecord::insert(&connection, &task("c2", Some("lead"))).expect("c2");
        TaskRecord::archive(&connection, "c2", 1).expect("archive c2");

        let refusal = archive_unless_children_open(&connection, "lead", 2).expect_err("refused");
        assert!(refusal.starts_with("has open children: c1."), "{refusal}");
        assert!(TaskRecord::get(&connection, "lead")
            .expect("lead")
            .archived_at_ms
            .is_none());

        archive_unless_children_open(&connection, "c1", 3).expect("a child archives");
        archive_unless_children_open(&connection, "lead", 4).expect("lead now archives");
    }
}

#[cfg(test)]
mod github_output_tests {
    use std::io::{Error, ErrorKind};
    use std::process::{ExitStatus, Output};

    use super::github_output;

    fn exit(code: u32) -> ExitStatus {
        #[cfg(windows)]
        {
            std::os::windows::process::ExitStatusExt::from_raw(code)
        }
        #[cfg(unix)]
        {
            std::os::unix::process::ExitStatusExt::from_raw(
                i32::try_from(code).expect("exit code fits in i32") << 8,
            )
        }
    }

    fn capped(code: u32, stdout: &str, stderr: &str, truncated: bool) -> brachio_process::Capped {
        brachio_process::Capped {
            output: Output {
                status: exit(code),
                stdout: stdout.as_bytes().to_vec(),
                stderr: stderr.as_bytes().to_vec(),
            },
            truncated,
        }
    }

    #[test]
    fn a_truncated_answer_is_an_error_not_cut_off_json() {
        let error = github_output(Ok(capped(0, "[[{\"a\"", "", true)), 2 << 20, "the reviews")
            .expect_err("fails");
        assert_eq!(
            error,
            "the reviews is larger than 2 MiB, so it was not loaded."
        );
    }

    #[test]
    fn a_timeout_a_missing_cli_and_a_failed_login_are_told_apart() {
        let timed_out = Error::new(ErrorKind::TimedOut, "timed out after 60 seconds");
        assert_eq!(
            github_output(Err(timed_out), 1, "the pull request").expect_err("fails"),
            "Loading the pull request from GitHub timed out after 60 seconds."
        );
        let missing = Error::new(ErrorKind::NotFound, "program not found");
        assert_eq!(
            github_output(Err(missing), 1, "x").expect_err("fails"),
            "The GitHub CLI (gh) is not installed or not on PATH."
        );
        assert_eq!(
            github_output(Ok(capped(1, "", "gh auth login required\n", false)), 1, "x")
                .expect_err("fails"),
            "gh auth login required"
        );
    }

    #[test]
    fn a_normal_answer_is_returned_whole() {
        assert_eq!(
            github_output(Ok(capped(0, "[]", "", false)), 1024, "x").expect("loads"),
            b"[]"
        );
    }
}

#[cfg(test)]
mod folder_task_deletion_tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use brachio_pty::{EventSink, SessionManager, SpawnSpec};
    use brachio_store::{open_in_memory, NewRepo, RepoRecord, TaskRecord};

    use super::{delete_task, Database, TaskWorktreeLocks};
    use crate::runtime_terminals::RuntimeTerminals;
    use crate::terminal::{SessionKeys, Terminals};

    fn folder_task_database() -> Database {
        let connection = open_in_memory().expect("open database");
        let repo = NewRepo {
            id: "f",
            name: "f",
            path: Path::new("/folder"),
            default_branch: "",
            added_at_ms: 0,
            is_git: false,
        };
        RepoRecord::insert(&connection, &repo).expect("insert repo");
        connection
            .execute_batch(
                "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, archived_at_ms)
                 VALUES ('t1', 'f', 'T', '', '', '/folder', 'active', 0, 1);",
            )
            .expect("insert task");
        Database(Mutex::new(connection))
    }

    fn long_running() -> SpawnSpec {
        let (program, args) = if cfg!(windows) {
            ("cmd.exe", vec!["/c", "ping", "-n", "60", "127.0.0.1"])
        } else {
            ("sleep", vec!["60"])
        };
        SpawnSpec {
            program: program.to_owned(),
            args: args.into_iter().map(str::to_owned).collect(),
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        }
    }

    #[test]
    fn a_folder_task_with_a_running_terminal_is_kept_until_it_stops() {
        let _runtime_off = crate::runtime_client::RuntimeOff::hold();
        let database = folder_task_database();
        let locks = TaskWorktreeLocks::default();
        let runtime = RuntimeTerminals::default();
        let manager = Arc::new(SessionManager::default());
        let in_app = Terminals(manager.clone(), SessionKeys::default());
        let sink: EventSink = Arc::new(|_| {});
        let live = manager.spawn(&long_running(), sink).expect("spawn");
        in_app.1.record(live, "t1:agent".to_owned());

        let refusal = tauri::async_runtime::block_on(delete_task(
            &database, &locks, &runtime, &in_app, "t1", "",
        ))
        .expect_err("refused");
        assert!(refusal.starts_with("terminal still running:"), "{refusal}");
        assert!(TaskRecord::get(&database.lock().expect("lock"), "t1").is_ok());

        manager.kill(live).expect("kill");
        let in_app = Terminals(Arc::new(SessionManager::default()), SessionKeys::default());
        tauri::async_runtime::block_on(delete_task(&database, &locks, &runtime, &in_app, "t1", ""))
            .expect("deleted once stopped");
        assert!(TaskRecord::get(&database.lock().expect("lock"), "t1").is_err());
    }
}

#[cfg(test)]
mod dropped_path_tests {
    use std::path::Path;

    use super::{dropped_path, strip_verbatim_prefix};

    #[test]
    fn strips_windows_verbatim_prefixes() {
        assert_eq!(strip_verbatim_prefix(r"\\?\C:\x\a.png"), r"C:\x\a.png");
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\x"),
            r"\\server\share\x"
        );
        assert_eq!(strip_verbatim_prefix("/home/a/b.txt"), "/home/a/b.txt");
        assert_eq!(strip_verbatim_prefix(r"C:\x"), r"C:\x");
    }

    #[test]
    fn dropped_paths_are_relative_inside_and_absolute_outside() {
        let worktree = tempfile::tempdir().expect("worktree");
        let elsewhere = tempfile::tempdir().expect("elsewhere");
        let root = worktree.path().canonicalize().expect("root");
        std::fs::create_dir_all(root.join("src/pics")).expect("dirs");
        std::fs::create_dir_all(root.join(".git")).expect("git");
        std::fs::write(root.join("src/a.txt"), "a").expect("file");
        std::fs::write(root.join(".git/config"), "c").expect("config");
        let outside = elsewhere.path().canonicalize().expect("outside");
        std::fs::write(outside.join("shot.png"), "p").expect("shot");
        std::fs::create_dir(outside.join("logs")).expect("logs");

        assert_eq!(
            dropped_path(&root, &root.join("src/a.txt")).as_deref(),
            Ok("src/a.txt")
        );
        assert_eq!(
            dropped_path(&root, &root.join("src/pics")).as_deref(),
            Ok("src/pics")
        );
        assert!(dropped_path(&root, &root.join(".git/config")).is_err());
        let shot = dropped_path(&root, &outside.join("shot.png")).expect("outside file");
        assert!(!shot.starts_with(r"\\?\"));
        assert!(Path::new(&shot).is_absolute() && shot.ends_with("shot.png"));
        let logs = dropped_path(&root, &outside.join("logs")).expect("outside folder");
        assert!(Path::new(&logs).is_absolute() && logs.ends_with("logs"));
    }
}

#[cfg(test)]
mod git_task_deletion_tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::{Arc, Mutex};

    use brachio_agent::AgentKind;
    use brachio_git::Git;
    use brachio_pty::SessionManager;
    use brachio_store::{open_in_memory, NewRepo, RepoRecord, TaskRecord};

    use super::{create_task, delete_task, CreateTaskRequest, Database, TaskWorktreeLocks};
    use crate::runtime_terminals::RuntimeTerminals;
    use crate::terminal::{SessionKeys, Terminals};

    fn git(folder: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(folder)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(args)
            .output()
            .expect("run git");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    #[test]
    fn work_added_after_the_preview_refuses_deletion_and_a_fresh_preview_deletes() {
        let root = tempfile::tempdir().expect("temp dir");
        let repo_path = root.path().join("repo");
        std::fs::create_dir(&repo_path).expect("make repo");
        git(&repo_path, &["init", "-b", "main"]);
        git(&repo_path, &["commit", "--allow-empty", "-m", "start"]);
        let connection = open_in_memory().expect("open database");
        let repo = NewRepo {
            id: "r",
            name: "repo",
            path: &repo_path,
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(&connection, &repo).expect("insert repo");
        let database = Database(Mutex::new(connection));
        let locks = TaskWorktreeLocks::default();
        let runtime = RuntimeTerminals::default();
        let in_app = Terminals(Arc::new(SessionManager::default()), SessionKeys::default());

        let created = tauri::async_runtime::block_on(create_task(
            &database,
            &locks,
            CreateTaskRequest {
                repo_id: "r".to_owned(),
                parent_task_id: None,
                title: "Work".to_owned(),
                prompt: String::new(),
                agent: AgentKind::ClaudeCode,
                base: None,
                codex_conversation_id: None,
                compare_group: None,
                max_open_children: None,
                scheduled: false,
                after: Vec::new(),
            },
        ))
        .expect("create task");
        let (id, worktree, branch) = (
            created.task.id.clone(),
            created.task.worktree_path.clone(),
            created.task.branch,
        );
        database
            .lock()
            .expect("lock")
            .execute("UPDATE tasks SET archived_at_ms = 1 WHERE id = ?1", [&id])
            .expect("archive");
        let shown = brachio_git::unsaved_work(&Git::new(&repo_path), &worktree, &branch)
            .expect("preview")
            .fingerprint;
        std::fs::write(worktree.join("late.txt"), "written after the preview").expect("write");

        let refusal = tauri::async_runtime::block_on(delete_task(
            &database, &locks, &runtime, &in_app, &id, &shown,
        ))
        .expect_err("stale fingerprint is refused");
        assert!(refusal.starts_with("task changed:"), "{refusal}");
        assert!(worktree.join("late.txt").exists());
        assert!(TaskRecord::get(&database.lock().expect("lock"), &id).is_ok());

        let fresh = brachio_git::unsaved_work(&Git::new(&repo_path), &worktree, &branch)
            .expect("second preview")
            .fingerprint;
        assert_ne!(fresh, shown);
        tauri::async_runtime::block_on(delete_task(
            &database, &locks, &runtime, &in_app, &id, &fresh,
        ))
        .expect("deleted after a fresh preview");
        assert!(!worktree.exists());
        assert!(TaskRecord::get(&database.lock().expect("lock"), &id).is_err());
        assert!(!git(&repo_path, &["branch", "--format=%(refname:short)"]).contains(&branch));
    }
}
