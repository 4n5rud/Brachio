//! File actions for a task's Files and Changes panels: open in an editor, rename, create, trash
//! and discard. Every path is relative to the task's folder and is checked here, not in the UI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use brachio_git::Git;
use serde::Serialize;
use tauri::State;

use brachio_store::TaskRecord;

use crate::task_file_ops::{canonical_root, clean_relative, resolve_dir};
use crate::workspace::{lock_open_git_task, Database, TaskWorktreeLocks};

fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Editors Brachio can start: the id the UI stores, the name shown, and the program looked up.
const EDITORS: [(&str, &str, &str); 3] = [
    ("vscode", "VS Code", "code"),
    ("cursor", "Cursor", "cursor"),
    ("zed", "Zed", "zed"),
];

#[derive(Serialize)]
pub struct EditorInfo {
    id: &'static str,
    name: &'static str,
}

/// The program file for `name` in `PATH`, with the extensions Windows launchers use.
fn find_program(name: &str, path_var: &std::ffi::OsStr) -> Option<PathBuf> {
    let extensions: &[&str] = if cfg!(windows) {
        &["exe", "cmd", "bat"]
    } else {
        &[""]
    };
    std::env::split_paths(path_var).find_map(|folder| {
        extensions.iter().find_map(|extension| {
            let file = if extension.is_empty() {
                folder.join(name)
            } else {
                folder.join(format!("{name}.{extension}"))
            };
            file.is_file().then_some(file)
        })
    })
}

/// The installed editors; the system default is always available and is not listed.
#[tauri::command]
pub async fn task_editors() -> Vec<EditorInfo> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = std::env::var_os("PATH").unwrap_or_default();
        EDITORS
            .iter()
            .filter(|(_, _, program)| find_program(program, &path).is_some())
            .map(|(id, name, _)| EditorInfo { id, name })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// An existing file or folder of the task, resolved through links, that stays inside it.
fn existing_inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let root = canonical_root(root)?;
    let target = root
        .join(clean_relative(relative)?)
        .canonicalize()
        .map_err(|_| "The item no longer exists.".to_owned())?;
    if !target.starts_with(&root) {
        return Err("The item is outside the task.".to_owned());
    }
    Ok(brachio_pty::without_verbatim_prefix(&target))
}

/// Opens a file in the chosen editor (`vscode`, `cursor`, `zed`) or, for anything else, the
/// system's default program. The editor is looked up by fixed name and gets the path as one
/// argument, never through a shell.
#[tauri::command]
pub async fn task_open_in_editor(
    database: State<'_, Database>,
    id: String,
    path: String,
    editor: Option<String>,
    line: Option<u32>,
) -> Result<(), String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        let file = existing_inside(&task.worktree_path, &path)?;
        let chosen = EDITORS
            .iter()
            .find(|(editor_id, _, _)| Some(*editor_id) == editor.as_deref());
        let program = chosen.map(|(_, _, program)| program);
        let mut command = match program {
            Some(program) => {
                let path_var = std::env::var_os("PATH").unwrap_or_default();
                let launcher = find_program(program, &path_var)
                    .ok_or_else(|| "That editor is not installed.".to_owned())?;
                let mut command = Command::new(launcher);
                command.args(editor_arguments(chosen.map(|(id, _, _)| *id), &file, line));
                command
            }
            None => crate::task_folder::open_command(&file),
        };
        hide_console(&mut command);
        command.spawn().map(drop).map_err(to_text)
    })
    .await
    .map_err(to_text)?
}

/// How each editor is told to open a file at a line; the system default cannot take a line.
fn editor_arguments(
    editor: Option<&str>,
    file: &Path,
    line: Option<u32>,
) -> Vec<std::ffi::OsString> {
    let at_line = |line: u32| format!("{}:{line}", file.display()).into();
    match (editor, line.filter(|line| *line > 0)) {
        (Some("vscode" | "cursor"), Some(line)) => vec!["--goto".into(), at_line(line)],
        (Some("zed"), Some(line)) => vec![at_line(line)],
        _ => vec![file.as_os_str().to_owned()],
    }
}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

/// A name for one file or folder: no separators, no dots-only, no control characters, not `.git`.
fn valid_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();
    let bad = trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.eq_ignore_ascii_case(".git")
        || trimmed.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\') || (cfg!(windows) && "<>:\"|?*".contains(c))
        });
    if bad {
        return Err("Enter a name without slashes or special characters.".to_owned());
    }
    Ok(trimmed)
}

/// The existing folder holding `relative`, inside the task, and the item's own name.
fn parent_and_name(
    root: &Path,
    relative: &str,
) -> Result<(PathBuf, PathBuf, std::ffi::OsString), String> {
    let root = canonical_root(root)?;
    let clean = clean_relative(relative)?;
    let name = clean
        .file_name()
        .ok_or_else(|| "Choose a file or folder.".to_owned())?
        .to_owned();
    let parent = root
        .join(&clean)
        .parent()
        .ok_or_else(|| "The item has no folder.".to_owned())?
        .canonicalize()
        .map_err(|_| "The item no longer exists.".to_owned())?;
    if !parent.starts_with(&root) {
        return Err("The item is outside the task.".to_owned());
    }
    fs::symlink_metadata(parent.join(&name))
        .map_err(|_| "The item no longer exists.".to_owned())?;
    Ok((root, parent, name))
}

fn relative_of(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(to_text)?;
    relative
        .components()
        .map(|part| part.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join("/"))
        .ok_or_else(|| "The path is not valid UTF-8.".to_owned())
}

fn rename_inside(root: &Path, from: &str, new_name: &str) -> Result<String, String> {
    let new_name = valid_name(new_name)?;
    let (root, parent, name) = parent_and_name(root, from)?;
    let destination = parent.join(new_name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(format!("\"{new_name}\" already exists."));
    }
    fs::rename(parent.join(&name), &destination).map_err(to_text)?;
    relative_of(&root, &destination)
}

fn create_inside(root: &Path, dir: &str, name: &str, folder: bool) -> Result<String, String> {
    let name = valid_name(name)?;
    let root = canonical_root(root)?;
    let target_dir = resolve_dir(&root, dir)?;
    let destination = target_dir.join(name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(format!("\"{name}\" already exists."));
    }
    if folder {
        fs::create_dir(&destination).map_err(to_text)?;
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(to_text)?;
    }
    relative_of(&root, &destination)
}

/// Renames a file or folder in place. Returns the new path.
#[tauri::command]
pub async fn task_file_rename(
    database: State<'_, Database>,
    id: String,
    from: String,
    new_name: String,
) -> Result<String, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        rename_inside(&task.worktree_path, &from, &new_name)
    })
    .await
    .map_err(to_text)?
}

/// Creates an empty file or a folder inside a folder of the task. Returns its path.
#[tauri::command]
pub async fn task_file_create(
    database: State<'_, Database>,
    id: String,
    dir: String,
    name: String,
    folder: bool,
) -> Result<String, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        create_inside(&task.worktree_path, &dir, &name, folder)
    })
    .await
    .map_err(to_text)?
}

/// Moves a file or folder to the system trash. Nothing is deleted for good. Returns whether
/// `task_file_restore` can bring it back, which the macOS trash does not allow.
#[tauri::command]
pub async fn task_file_trash(
    database: State<'_, Database>,
    id: String,
    path: String,
) -> Result<bool, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (_, parent, name) = parent_and_name(&task.worktree_path, &path)?;
        trash::delete(parent.join(name)).map_err(to_text)?;
        Ok(cfg!(not(target_os = "macos")))
    })
    .await
    .map_err(to_text)?
}

/// Puts a file or folder that `task_file_trash` removed back where it was.
#[tauri::command]
pub async fn task_file_restore(
    database: State<'_, Database>,
    id: String,
    path: String,
) -> Result<(), String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        let original = canonical_root(&task.worktree_path)?.join(clean_relative(&path)?);
        restore_from_trash(&original)
    })
    .await
    .map_err(to_text)?
}

#[cfg(not(target_os = "macos"))]
fn restore_from_trash(original: &Path) -> Result<(), String> {
    let same = |item: &trash::TrashItem| {
        let found = item.original_path();
        if cfg!(windows) {
            // The trash lists plain drive paths; a canonical path carries the `\\?\` prefix.
            let plain = |path: &Path| {
                path.to_string_lossy()
                    .trim_start_matches(r"\\?\")
                    .to_owned()
            };
            plain(&found).eq_ignore_ascii_case(&plain(original))
        } else {
            found == original
        }
    };
    let newest = trash::os_limited::list()
        .map_err(to_text)?
        .into_iter()
        .filter(same)
        .max_by_key(|item| item.time_deleted)
        .ok_or_else(|| "It is no longer in the trash.".to_owned())?;
    trash::os_limited::restore_all([newest]).map_err(to_text)
}

#[cfg(target_os = "macos")]
fn restore_from_trash(_original: &Path) -> Result<(), String> {
    Err("Restore it from the Trash in Finder.".to_owned())
}

/// Throws away a file's changes: a tracked file goes back to `HEAD`; a new file goes to the trash.
#[tauri::command]
pub async fn task_discard_file(
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    id: String,
    path: String,
) -> Result<(), String> {
    let (task, _guard) = lock_open_git_task(&database, &worktrees, &id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let clean = clean_relative(&path)?;
        let text = relative_of(Path::new(""), &clean)?;
        let git = Git::new(&task.worktree_path);
        if brachio_git::in_head(&git, &text).map_err(to_text)? {
            return brachio_git::discard_tracked(&git, &text).map_err(to_text);
        }
        // A new file: drop it from the index if staged, then move it to the trash.
        let _ = git.output(&["--literal-pathspecs", "rm", "--cached", "-q", "--", &text]);
        let (_, parent, name) = parent_and_name(&task.worktree_path, &text)?;
        trash::delete(parent.join(name)).map_err(to_text)
    })
    .await
    .map_err(to_text)?
}

/// Reverses one hunk of a file in the working tree.
#[tauri::command]
pub async fn task_discard_hunk(
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    id: String,
    patch: String,
) -> Result<(), String> {
    let (task, _guard) = lock_open_git_task(&database, &worktrees, &id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        brachio_git::discard_hunk(&Git::new(&task.worktree_path), &patch).map_err(to_text)
    })
    .await
    .map_err(to_text)?
}

#[cfg(test)]
mod tests {
    use super::{create_inside, editor_arguments, find_program, rename_inside, valid_name};
    use std::fs;

    #[test]
    fn names_with_separators_or_dots_are_refused() {
        for bad in ["", " ", ".", "..", "a/b", "a\\b", ".git", "tab\there"] {
            assert!(valid_name(bad).is_err(), "{bad:?}");
        }
        assert_eq!(valid_name(" ok name.txt "), Ok("ok name.txt"));
    }

    #[test]
    fn renames_in_place_and_refuses_a_clash_or_escape() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("src")).expect("src");
        fs::write(dir.path().join("src/a.txt"), "a").expect("a");
        fs::write(dir.path().join("src/b.txt"), "b").expect("b");
        assert_eq!(
            rename_inside(dir.path(), "src/a.txt", "c.txt").as_deref(),
            Ok("src/c.txt")
        );
        assert!(rename_inside(dir.path(), "src/c.txt", "b.txt").is_err());
        assert!(rename_inside(dir.path(), "src/c.txt", "../x.txt").is_err());
        assert!(rename_inside(dir.path(), "../src/c.txt", "d.txt").is_err());
        assert!(rename_inside(dir.path(), "missing.txt", "d.txt").is_err());
        assert!(dir.path().join("src/b.txt").is_file() && dir.path().join("src/c.txt").is_file());
    }

    #[test]
    fn creates_files_and_folders_without_replacing() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("src")).expect("src");
        assert_eq!(
            create_inside(dir.path(), "src", "n.txt", false).as_deref(),
            Ok("src/n.txt")
        );
        assert_eq!(
            create_inside(dir.path(), "", "pkg", true).as_deref(),
            Ok("pkg")
        );
        assert!(dir.path().join("pkg").is_dir());
        fs::write(dir.path().join("src/n.txt"), "keep").expect("write");
        assert!(create_inside(dir.path(), "src", "n.txt", false).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("src/n.txt")).expect("read"),
            "keep"
        );
        assert!(create_inside(dir.path(), "..", "x", false).is_err());
        assert!(create_inside(dir.path(), "nope", "x", false).is_err());
    }

    #[test]
    fn editors_get_the_line_in_their_own_form() {
        let file = std::path::Path::new("src/a.rs");
        let args = |editor, line| editor_arguments(editor, file, line);
        assert_eq!(args(Some("vscode"), Some(7)), ["--goto", "src/a.rs:7"]);
        assert_eq!(args(Some("zed"), Some(7)), ["src/a.rs:7"]);
        assert_eq!(args(Some("cursor"), None), ["src/a.rs"]);
        assert_eq!(args(None, Some(7)), ["src/a.rs"]);
        assert_eq!(args(Some("vscode"), Some(0)), ["src/a.rs"]);
    }

    #[test]
    fn finds_a_program_on_the_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let name = if cfg!(windows) {
            "fake-editor.exe"
        } else {
            "fake-editor"
        };
        fs::write(dir.path().join(name), "").expect("write");
        let path = std::env::join_paths([dir.path()]).expect("join");
        assert!(find_program("fake-editor", &path).is_some());
        assert!(find_program("other-editor", &path).is_none());
    }
}
