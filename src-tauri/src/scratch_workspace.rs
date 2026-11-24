//! Scratch tasks: an agent task in a temporary folder with no git repository, which can later
//! become a project. A scratch task is a folder task whose folder Brachio made, under the app's
//! own `scratch` folder, so unconverted ones can be listed and cleaned up.

use std::fs;
use std::path::{Path, PathBuf};

use brachio_agent::AgentKind;
use brachio_git::Git;
use brachio_pty::without_verbatim_prefix;
use brachio_store::{NewRepo, RepoRecord, TaskRecord};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::project_add::{checked_target, init_repository};
use crate::runtime_terminals::RuntimeTerminals;
use crate::terminal::Terminals;
use crate::workspace::{
    create_folder_task, ensure_no_live_terminals, live_task_terminals, new_id, now_ms,
    CreateTaskRequest, CreatedTask, Database, TaskWorktreeLocks,
};

const SCRATCH_FOLDER: &str = "scratch";
const DEFAULT_TITLE: &str = "Scratch";

/// A scratch task as the window asks for it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScratchRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    pub agent: AgentKind,
}

/// Where a scratch folder goes when it becomes a project.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub repo_id: String,
    pub parent: PathBuf,
    pub name: String,
}

/// One folder under the scratch folder: a project that was not converted yet, or a folder left
/// behind without one.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScratchFolder {
    /// The project registered for the folder, if there is one.
    pub repo_id: Option<String>,
    pub name: String,
    pub path: String,
    /// Titles of the project's tasks.
    pub tasks: Vec<String>,
    /// The folder itself is gone; only the project remains.
    pub missing: bool,
}

/// The folder scratch tasks live in, created when missing.
fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("could not create {}: {error}", root.display()))?;
    root.canonicalize()
        .map(|path| without_verbatim_prefix(&path))
        .map_err(|error| error.to_string())
}

fn folder_slug(title: &str) -> String {
    let slug: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug: String = slug.chars().take(24).collect();
    if slug.is_empty() {
        "scratch".to_owned()
    } else {
        slug
    }
}

/// Makes a scratch folder, registers it as a folder project and starts one task in it. Nothing
/// is left behind when a step fails.
pub(crate) fn create_scratch(
    database: &Database,
    root: &Path,
    request: &ScratchRequest,
) -> Result<CreatedTask, String> {
    let root = canonical_root(root)?;
    let title = match request.title.trim() {
        "" => DEFAULT_TITLE,
        text => text,
    };
    let id = new_id();
    let folder = root.join(format!("{}-{}", folder_slug(title), &id[..6]));
    fs::create_dir(&folder)
        .map_err(|error| format!("could not create {}: {error}", folder.display()))?;
    let registered = register_scratch(database, &folder, title, &id, request);
    if registered.is_err() {
        let _ = fs::remove_dir_all(&folder);
    }
    registered
}

fn register_scratch(
    database: &Database,
    folder: &Path,
    title: &str,
    id: &str,
    request: &ScratchRequest,
) -> Result<CreatedTask, String> {
    let repo = {
        let connection = database.lock()?;
        RepoRecord::insert(
            &connection,
            &NewRepo {
                id,
                name: title,
                path: folder,
                default_branch: "",
                added_at_ms: now_ms(),
                is_git: false,
            },
        )
        .map_err(|error| error.to_string())?
    };
    let task = CreateTaskRequest {
        repo_id: repo.id.clone(),
        parent_task_id: None,
        title: title.to_owned(),
        prompt: request.prompt.clone(),
        agent: request.agent,
        base: None,
        codex_conversation_id: None,
        compare_group: None,
        profile: None,
        max_open_children: None,
        scheduled: false,
        after: Vec::new(),
    };
    create_folder_task(database, &repo, &task).inspect_err(|_| {
        if let Ok(connection) = database.lock() {
            let _ = RepoRecord::delete(&connection, &repo.id);
        }
    })
}

fn is_inside(root: &Path, path: &Path) -> bool {
    path.parent() == Some(root)
}

/// The scratch folders on disk and the scratch projects, merged by path.
pub(crate) fn list_scratch(database: &Database, root: &Path) -> Result<Vec<ScratchFolder>, String> {
    let root = canonical_root(root)?;
    let connection = database.lock()?;
    let mut found: Vec<ScratchFolder> = Vec::new();
    for repo in RepoRecord::list(&connection).map_err(|error| error.to_string())? {
        if repo.is_git || !is_inside(&root, &repo.path) {
            continue;
        }
        let tasks = TaskRecord::ids_for_repo(&connection, &repo.id)
            .map_err(|error| error.to_string())?
            .iter()
            .filter_map(|id| TaskRecord::get(&connection, id).ok())
            .map(|task| task.title)
            .collect();
        found.push(ScratchFolder {
            repo_id: Some(repo.id),
            name: repo.name,
            missing: !repo.path.is_dir(),
            path: repo.path.display().to_string(),
            tasks,
        });
    }
    let entries = fs::read_dir(&root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let shown = path.display().to_string();
        if path.is_dir() && !found.iter().any(|folder| folder.path == shown) {
            found.push(ScratchFolder {
                repo_id: None,
                name: entry.file_name().to_string_lossy().into_owned(),
                path: shown,
                tasks: Vec::new(),
                missing: false,
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

/// Ids of the tasks in the scratch project at `path`.
fn scratch_task_ids(database: &Database, path: &Path) -> Result<Vec<String>, String> {
    let connection = database.lock()?;
    match RepoRecord::find_by_path(&connection, path).map_err(|error| error.to_string())? {
        Some(repo) => TaskRecord::ids_for_repo(&connection, &repo.id).map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

/// Moves the folder, across drives too.
fn move_folder(from: &Path, to: &Path) -> Result<(), String> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_then_remove(from, to, |path| fs::remove_dir_all(path))
}

/// Copies `from` to `to`, then removes `from`. When either step fails, `from` is left whole
/// and `to` is removed, so a retry starts clean.
fn copy_then_remove(
    from: &Path,
    to: &Path,
    remove: impl Fn(&Path) -> std::io::Result<()>,
) -> Result<(), String> {
    if let Err(error) = copy_folder(from, to) {
        let _ = fs::remove_dir_all(to);
        return Err(format!("could not move {}: {error}", from.display()));
    }
    if let Err(error) = remove(from) {
        // Removing may have deleted part of the source; the copy has every file, so put them back.
        let restored = copy_folder(to, from);
        if restored.is_ok() {
            let _ = fs::remove_dir_all(to);
        }
        return Err(match restored {
            Ok(()) => format!("could not remove {}: {error}", from.display()),
            Err(restore_error) => format!(
                "could not remove {}: {error}; the full copy is kept at {} ({restore_error})",
                from.display(),
                to.display()
            ),
        });
    }
    Ok(())
}

fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            copy_link(&entry.path(), &target)?;
        } else if kind.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Recreates a symbolic link instead of following it.
fn copy_link(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.symlink_metadata().is_ok() {
        return Ok(());
    }
    let destination = fs::read_link(from)?;
    #[cfg(unix)]
    return std::os::unix::fs::symlink(destination, to);
    #[cfg(windows)]
    return if from.is_dir() {
        std::os::windows::fs::symlink_dir(destination, to)
    } else {
        std::os::windows::fs::symlink_file(destination, to)
    };
}

/// Moves the scratch folder of project `repo_id` to `parent/name`, runs `git init` there and
/// registers it as a git project. Failing at any step puts the folder back and leaves the
/// project as it was. The caller has checked that no terminal runs in it.
pub(crate) fn convert_scratch(
    database: &Database,
    root: &Path,
    repo_id: &str,
    parent: &Path,
    name: &str,
) -> Result<RepoRecord, String> {
    let root = canonical_root(root)?;
    let repo = RepoRecord::get(&*database.lock()?, repo_id).map_err(|error| error.to_string())?;
    if repo.is_git || !is_inside(&root, &repo.path) {
        return Err("Only a scratch folder can be turned into a project.".to_owned());
    }
    if !repo.path.is_dir() {
        return Err("The scratch folder is gone, so there is nothing to move.".to_owned());
    }
    let name = name.trim();
    let (target, existed) = checked_target(parent, name)?;
    let parent_real = parent.canonicalize().map_err(|error| error.to_string())?;
    if without_verbatim_prefix(&parent_real).starts_with(&repo.path) {
        return Err("Choose a folder outside the scratch folder.".to_owned());
    }
    if existed {
        fs::remove_dir(&target)
            .map_err(|error| format!("could not use {}: {error}", target.display()))?;
    }
    move_folder(&repo.path, &target)?;
    let back = |reason: String| {
        let _ = move_folder(&target, &repo.path);
        reason
    };
    let had_git = target.join(".git").exists();
    if let Err(error) = init_repository(&target, true) {
        return Err(back(error));
    }
    let branch = Git::new(&target)
        .run(&["symbolic-ref", "--short", "HEAD"])
        .map_err(|error| {
            if !had_git {
                let _ = fs::remove_dir_all(target.join(".git"));
            }
            back(error.to_string())
        })?;
    let canonical =
        without_verbatim_prefix(&target.canonicalize().unwrap_or_else(|_| target.clone()));
    let converted = database.lock().and_then(|mut connection| {
        RepoRecord::become_git(&mut connection, repo_id, &canonical, &branch)
            .map_err(|error| error.to_string())
    });
    converted.map_err(|error| {
        if !had_git {
            let _ = fs::remove_dir_all(target.join(".git"));
        }
        back(error)
    })
}

/// Deletes a scratch folder and its project and tasks. The caller has checked that no terminal
/// runs in it.
pub(crate) fn discard_scratch(database: &Database, root: &Path, path: &Path) -> Result<(), String> {
    let root = canonical_root(root)?;
    if !is_inside(&root, path) {
        return Err("That folder is not a scratch folder.".to_owned());
    }
    {
        let connection = database.lock()?;
        if let Some(repo) =
            RepoRecord::find_by_path(&connection, path).map_err(|error| error.to_string())?
        {
            if repo.is_git {
                return Err("This folder is a project now, not scratch.".to_owned());
            }
        }
    }
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("could not remove {}: {error}", path.display())),
    }
    let mut connection = database.lock()?;
    let Some(repo) =
        RepoRecord::find_by_path(&connection, path).map_err(|error| error.to_string())?
    else {
        return Ok(());
    };
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for id in TaskRecord::ids_for_repo(&transaction, &repo.id).map_err(|e| e.to_string())? {
        brachio_store::forget_task_messages(&transaction, &id).map_err(|e| e.to_string())?;
        TaskRecord::delete(&transaction, &id).map_err(|e| e.to_string())?;
    }
    RepoRecord::delete(&transaction, &repo.id).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|error| error.to_string())
}

fn scratch_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|folder| folder.join(SCRATCH_FOLDER))
        .map_err(|error| error.to_string())
}

/// Starts a scratch task: an agent in a new temporary folder with no git repository.
#[tauri::command]
pub async fn scratch_create(
    app: AppHandle,
    database: State<'_, Database>,
    request: ScratchRequest,
) -> Result<CreatedTask, String> {
    let root = scratch_root(&app)?;
    create_scratch(&database, &root, &request)
}

/// Scratch folders that were not turned into projects, for cleaning up.
#[tauri::command]
pub async fn scratch_list(
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<Vec<ScratchFolder>, String> {
    let root = scratch_root(&app)?;
    list_scratch(&database, &root)
}

async fn ensure_stopped(
    database: &Database,
    terminals: &RuntimeTerminals,
    in_app: &Terminals,
    task_ids: &[String],
) -> Result<(), String> {
    for id in task_ids {
        ensure_no_live_terminals(live_task_terminals(database, terminals, in_app, id).await)?;
    }
    Ok(())
}

/// Moves a scratch folder to `parent/name`, runs `git init` and registers it as a project.
#[tauri::command]
pub async fn scratch_convert(
    app: AppHandle,
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    terminals: State<'_, RuntimeTerminals>,
    in_app: State<'_, Terminals>,
    request: ConvertRequest,
) -> Result<RepoRecord, String> {
    let ConvertRequest {
        repo_id,
        parent,
        name,
    } = request;
    let root = scratch_root(&app)?;
    let _guard = worktrees.acquire(&repo_id).await?;
    let path = RepoRecord::get(&*database.lock()?, &repo_id)
        .map_err(|error| error.to_string())?
        .path;
    ensure_stopped(
        &database,
        &terminals,
        &in_app,
        &scratch_task_ids(&database, &path)?,
    )
    .await?;
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let database = handle.state::<Database>();
        convert_scratch(&database, &root, &repo_id, &parent, &name)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Deletes a scratch folder with its project and tasks.
#[tauri::command]
pub async fn scratch_discard(
    app: AppHandle,
    database: State<'_, Database>,
    terminals: State<'_, RuntimeTerminals>,
    in_app: State<'_, Terminals>,
    path: PathBuf,
) -> Result<(), String> {
    let root = scratch_root(&app)?;
    ensure_stopped(
        &database,
        &terminals,
        &in_app,
        &scratch_task_ids(&database, &path)?,
    )
    .await?;
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let database = handle.state::<Database>();
        discard_scratch(&database, &root, &path)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use brachio_store::open_in_memory;

    use super::*;

    fn database() -> Database {
        Database(Mutex::new(open_in_memory().unwrap()))
    }

    fn request(title: &str) -> ScratchRequest {
        ScratchRequest {
            title: title.to_owned(),
            prompt: "try it".to_owned(),
            agent: AgentKind::ClaudeCode,
        }
    }

    fn repo_of(database: &Database, task: &CreatedTask) -> RepoRecord {
        RepoRecord::get(&database.lock().unwrap(), &task.task.repo_id).unwrap()
    }

    #[test]
    fn a_failed_removal_after_copying_leaves_the_source_whole_and_the_target_gone() {
        let root = tempfile::tempdir().unwrap();
        let from = root.path().join("from");
        fs::create_dir_all(from.join("sub")).unwrap();
        fs::write(from.join("a.txt"), "a").unwrap();
        fs::write(from.join("sub").join("b.txt"), "b").unwrap();
        let to = root.path().join("to");
        let partly_removing = |path: &Path| {
            fs::remove_file(path.join("a.txt"))?;
            Err(std::io::Error::other("locked"))
        };
        let error = copy_then_remove(&from, &to, partly_removing).unwrap_err();
        assert!(error.contains("could not remove"), "{error}");
        assert_eq!(fs::read_to_string(from.join("a.txt")).unwrap(), "a");
        assert_eq!(
            fs::read_to_string(from.join("sub").join("b.txt")).unwrap(),
            "b"
        );
        assert!(!to.exists());
    }

    #[test]
    fn a_scratch_task_runs_in_a_new_folder_without_git() {
        let database = database();
        let root = tempfile::tempdir().unwrap();
        let created =
            create_scratch(&database, &root.path().join("scratch"), &request("My idea")).unwrap();
        let repo = repo_of(&database, &created);
        assert!(!repo.is_git);
        assert!(repo.path.is_dir() && !repo.path.join(".git").exists());
        assert_eq!(created.task.worktree_path, repo.path);
        assert!(created.task.branch.is_empty());
        let listed = list_scratch(&database, &root.path().join("scratch")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].tasks, vec!["My idea".to_owned()]);
    }

    #[test]
    fn turning_it_into_a_project_moves_the_files_and_starts_a_repository() {
        let database = database();
        let root = tempfile::tempdir().unwrap();
        let scratch = root.path().join("scratch");
        let created = create_scratch(&database, &scratch, &request("Idea")).unwrap();
        let repo = repo_of(&database, &created);
        fs::write(repo.path.join("notes.txt"), "hello").unwrap();
        let parent = tempfile::tempdir().unwrap();

        let project =
            convert_scratch(&database, &scratch, &repo.id, parent.path(), "idea").unwrap();

        let target = parent.path().join("idea");
        assert!(project.is_git && project.id == repo.id);
        assert_eq!(
            fs::read_to_string(target.join("notes.txt")).unwrap(),
            "hello"
        );
        assert!(target.join(".git").exists());
        assert!(!repo.path.exists(), "the scratch folder is gone");
        let task = TaskRecord::get(&database.lock().unwrap(), &created.task.id).unwrap();
        assert_eq!(task.worktree_path, project.path);
        assert_eq!(task.branch, project.default_branch);
        assert!(list_scratch(&database, &scratch).unwrap().is_empty());
    }

    #[test]
    fn a_failed_conversion_puts_the_folder_back_unchanged() {
        let database = database();
        let root = tempfile::tempdir().unwrap();
        let scratch = root.path().join("scratch");
        let created = create_scratch(&database, &scratch, &request("Idea")).unwrap();
        let repo = repo_of(&database, &created);
        fs::write(repo.path.join("notes.txt"), "hello").unwrap();
        let parent = tempfile::tempdir().unwrap();
        // Another project already sits at the target, so registering the move fails last.
        let taken = parent.path().join("idea");
        fs::create_dir(&taken).unwrap();
        RepoRecord::insert(
            &database.lock().unwrap(),
            &NewRepo {
                id: "other",
                name: "other",
                path: &taken
                    .canonicalize()
                    .map(|p| without_verbatim_prefix(&p))
                    .unwrap(),
                default_branch: "main",
                added_at_ms: 0,
                is_git: true,
            },
        )
        .unwrap();

        let failed = convert_scratch(&database, &scratch, &repo.id, parent.path(), "idea");

        assert!(failed.is_err());
        assert_eq!(
            fs::read_to_string(repo.path.join("notes.txt")).unwrap(),
            "hello"
        );
        assert!(!repo.path.join(".git").exists());
        let kept = RepoRecord::get(&database.lock().unwrap(), &repo.id).unwrap();
        assert!(!kept.is_git && kept.path == repo.path);
    }

    #[test]
    fn only_scratch_folders_can_be_converted_or_discarded() {
        let database = database();
        let root = tempfile::tempdir().unwrap();
        let scratch = root.path().join("scratch");
        let own = tempfile::tempdir().unwrap();
        let folder = RepoRecord::insert(
            &database.lock().unwrap(),
            &NewRepo {
                id: "mine",
                name: "mine",
                path: own.path(),
                default_branch: "",
                added_at_ms: 0,
                is_git: false,
            },
        )
        .unwrap();
        let parent = tempfile::tempdir().unwrap();
        assert!(convert_scratch(&database, &scratch, &folder.id, parent.path(), "x").is_err());
        assert!(discard_scratch(&database, &scratch, own.path()).is_err());
        assert!(own.path().is_dir());
    }

    #[test]
    fn discarding_removes_the_folder_project_and_tasks_and_a_left_over_folder_is_listed() {
        let database = database();
        let root = tempfile::tempdir().unwrap();
        let scratch = root.path().join("scratch");
        let created = create_scratch(&database, &scratch, &request("Idea")).unwrap();
        let repo = repo_of(&database, &created);
        let orphan = canonical_root(&scratch).unwrap().join("left-over");
        fs::create_dir(&orphan).unwrap();
        assert_eq!(list_scratch(&database, &scratch).unwrap().len(), 2);

        discard_scratch(&database, &scratch, &repo.path).unwrap();
        discard_scratch(&database, &scratch, &repo.path).unwrap();
        discard_scratch(&database, &scratch, &orphan).unwrap();

        assert!(!repo.path.exists() && !orphan.exists());
        assert!(RepoRecord::list(&database.lock().unwrap())
            .unwrap()
            .is_empty());
        assert!(TaskRecord::get(&database.lock().unwrap(), &created.task.id).is_err());
        assert!(list_scratch(&database, &scratch).unwrap().is_empty());
    }
}
