//! IPC commands for repositories and tasks: add a repository, create a task
//! with its own worktree and branch, and list what is stored.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use brachio_agent::AgentKind;
use brachio_git::{predict_conflict, AddWorktree, Git, PullRequest};
use brachio_store::{NewRepo, NewTask, RepoRecord, TaskRecord};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

/// App state that owns the database connection.
pub struct Database(pub Mutex<Connection>);

impl Database {
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
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

/// A task that was just created.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedTask {
    #[serde(flatten)]
    task: TaskRecord,
    /// Commands from the repository's `brachio.toml` that the user is asked to run in the new
    /// worktree. Nothing runs without the user's say-so.
    setup_commands: Vec<String>,
    /// The file copy error that can be retried separately.
    setup_copy_error: Option<String>,
}

/// Creates a worktree on a fresh branch and stores the task. Files named in the repository's
/// `brachio.toml` are copied into the worktree, and its setup commands are returned.
///
/// git runs off the UI thread, since `worktree add` checks out files.
#[tauri::command]
pub async fn task_create(
    database: State<'_, Database>,
    request: CreateTaskRequest,
) -> Result<CreatedTask, String> {
    let repo = RepoRecord::get(&*database.lock()?, &request.repo_id).map_err(to_text)?;
    // Read first, so a broken file stops the task before anything is created.
    let setup = brachio_git::read_setup(&repo.path).map_err(to_text)?;
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
    let task = TaskRecord::insert(&*database.lock()?, &task).map_err(to_text)?;
    let setup_copy_error = brachio_git::copy_setup_files(&repo.path, &path, &setup)
        .err()
        .map(to_text);
    Ok(CreatedTask {
        task,
        setup_commands: setup.run,
        setup_copy_error,
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
    TaskRecord::archive(&*database.lock()?, &id, now_ms()).map_err(to_text)
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

/// Converts files from a native drop into worktree-relative paths for the agent draft.
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
            if !path.is_absolute() {
                return Err("Drop files from inside the current worktree.".to_owned());
            }
            let file = path.canonicalize().map_err(to_text)?;
            let relative = file
                .strip_prefix(&root)
                .map_err(|_| "Drop files from inside the current worktree.".to_owned())?;
            if !file.is_file()
                || relative
                    .components()
                    .any(|component| component.as_os_str().eq_ignore_ascii_case(".git"))
            {
                return Err("Only regular worktree files can be dropped.".to_owned());
            }
            let parts = relative
                .components()
                .map(|component| component.as_os_str().to_str())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| "The dropped path is not valid UTF-8.".to_owned())?;
            let path = parts.join("/");
            if path.chars().any(char::is_control) || (cfg!(windows) && path.contains(':')) {
                return Err("The dropped path contains an unsupported character.".to_owned());
            }
            if !files.contains(&path) {
                files.push(path);
            }
        }
        Ok(files)
    })
    .await
    .map_err(to_text)?
}

/// Lists the files of a task's worktree, tracked and not ignored, for quick open.
#[tauri::command]
pub async fn task_files(database: State<'_, Database>, id: String) -> Result<Vec<String>, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        brachio_git::list_files(&Git::new(&task.worktree_path))
    })
    .await
    .map_err(to_text)?
    .map_err(to_text)
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
}

/// Loads an archived task and the repository it belongs to. Only archived tasks can be deleted,
/// so no terminal is still using the worktree.
fn archived_task(database: &Database, id: &str) -> Result<(TaskRecord, RepoRecord), String> {
    let connection = database.lock()?;
    let task = TaskRecord::get(&connection, id).map_err(to_text)?;
    if task.archived_at_ms.is_none() {
        return Err("Archive the task before deleting it.".to_owned());
    }
    let repo = RepoRecord::get(&connection, &task.repo_id).map_err(to_text)?;
    Ok((task, repo))
}

/// Counts the work that deleting an archived task would lose, so the user can confirm it.
#[tauri::command]
pub async fn task_delete_preview(
    database: State<'_, Database>,
    id: String,
) -> Result<TaskLoss, String> {
    let (task, repo) = archived_task(&database, &id)?;
    let work = tauri::async_runtime::spawn_blocking(move || {
        brachio_git::unsaved_work(&Git::new(&repo.path), &task.worktree_path, &task.branch)
    })
    .await
    .map_err(to_text)?
    .map_err(to_text)?;
    Ok(TaskLoss {
        uncommitted_files: work.uncommitted_files,
        unpushed_commits: work.unpushed_commits,
    })
}

/// Deletes an archived task for good: its worktree folder, its branch and its record. The caller
/// has shown `task_delete_preview` to the user.
#[tauri::command]
pub async fn task_delete(database: State<'_, Database>, id: String) -> Result<(), String> {
    let (task, repo) = archived_task(&database, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        brachio_git::discard_worktree(&Git::new(&repo.path), &task.worktree_path, &task.branch)
    })
    .await
    .map_err(to_text)?
    .map_err(to_text)?;
    TaskRecord::delete(&*database.lock()?, &id).map_err(to_text)
}

/// Changes in a task worktree relative to its base.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskChanges {
    patch: String,
    untracked: Vec<String>,
    staged: Vec<String>,
    conflicts: Vec<String>,
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
        let untracked = git_paths(&git, &["ls-files", "--others", "--exclude-standard", "-z"])?;
        let conflicts = git_paths(
            &git,
            &[
                "diff",
                "--cached",
                "--diff-filter=U",
                "--name-only",
                "-z",
                "--",
            ],
        )?;
        let staged = git_paths(
            &git,
            &[
                "diff",
                "--cached",
                "--no-renames",
                "--name-only",
                "-z",
                "--",
            ],
        )?;
        Ok(TaskChanges {
            patch,
            untracked,
            staged,
            conflicts,
            truncated,
        })
    })
    .await
    .map_err(to_text)?
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
}

/// Checks how the task's committed HEAD would merge into the local default branch.
#[tauri::command]
pub async fn task_merge_preview(
    database: State<'_, Database>,
    id: String,
) -> Result<TaskMergePreview, String> {
    let (worktree_path, base_branch) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &id).map_err(to_text)?;
        let repo = RepoRecord::get(&connection, &task.repo_id).map_err(to_text)?;
        (task.worktree_path, repo.default_branch)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let base_ref = format!("refs/heads/{base_branch}");
        let report =
            predict_conflict(&Git::new(&worktree_path), &base_ref, "HEAD").map_err(to_text)?;
        Ok(TaskMergePreview {
            base_branch,
            conflicts: report.conflicted_files,
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
    /// Push the task branch to origin.
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
    id: String,
    request: TaskGitAction,
) -> Result<Option<String>, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
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
                git.run(&["push", "--", "origin", &refspec])
            }
            TaskGitAction::CreatePr { .. } => unreachable!("handled before the match"),
        }
        .map_err(to_text)?;
        Ok(None)
    })
    .await
    .map_err(to_text)?
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
    tauri::async_runtime::spawn_blocking(move || {
        let output = std::process::Command::new("gh")
            .current_dir(&task.worktree_path)
            .args([
                "pr",
                "view",
                "--json",
                "url,statusCheckRollup,reviews,comments",
                "--",
            ])
            .arg(&task.branch)
            .output()
            .map_err(to_text)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        let mut feedback: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(to_text)?;
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
    let output = std::process::Command::new("gh")
        .current_dir(worktree)
        .args([
            "api",
            "--hostname",
            host,
            "--paginate",
            "--slurp",
            &endpoint,
        ])
        .output()
        .map_err(to_text)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let pages: Vec<Vec<serde_json::Value>> =
        serde_json::from_slice(&output.stdout).map_err(to_text)?;
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
