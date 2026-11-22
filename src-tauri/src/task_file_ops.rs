//! Moves and copies files inside a task's worktree or folder, for drag and drop in the Files panel.

use std::fs;
use std::path::{Component, Path, PathBuf};

use brachio_store::TaskRecord;
use tauri::State;

use crate::workspace::Database;

/// Most entries one dropped folder may bring in, so a stray drop cannot fill the disk.
const MAX_COPY_ENTRIES: usize = 10_000;
const MAX_DROPPED_SOURCES: usize = 16;

fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Moves a file or folder into another folder of the task, by rename. Returns the new path.
#[tauri::command]
pub async fn task_move_path(
    database: State<'_, Database>,
    id: String,
    from: String,
    to_dir: String,
) -> Result<String, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || move_inside(&task.worktree_path, &from, &to_dir))
        .await
        .map_err(to_text)?
}

/// Names in `to_dir` that copying the dropped paths there would replace.
#[tauri::command]
pub async fn task_copy_conflicts(
    database: State<'_, Database>,
    id: String,
    sources: Vec<PathBuf>,
    to_dir: String,
) -> Result<Vec<String>, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        let target = resolve_dir(&task.worktree_path, &to_dir)?;
        let mut clashes = Vec::new();
        for source in checked_sources(&sources)? {
            let name = source_name(&source)?;
            if fs::symlink_metadata(target.join(&name)).is_ok() {
                clashes.push(name.to_string_lossy().into_owned());
            }
        }
        Ok(clashes)
    })
    .await
    .map_err(to_text)?
}

/// Copies files and folders from anywhere on disk into a folder of the task. Existing files are
/// replaced only with `overwrite`; an existing folder is never replaced.
#[tauri::command]
pub async fn task_copy_in(
    database: State<'_, Database>,
    id: String,
    sources: Vec<PathBuf>,
    to_dir: String,
    overwrite: bool,
) -> Result<Vec<String>, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        copy_inside(&task.worktree_path, &sources, &to_dir, overwrite)
    })
    .await
    .map_err(to_text)?
}

/// A path relative to the root, with `/`, that holds no `..`, drive, `.git` or control character.
pub(crate) fn clean_relative(path: &str) -> Result<PathBuf, String> {
    if path.chars().any(char::is_control) || (cfg!(windows) && path.contains(':')) {
        return Err("The path contains an unsupported character.".to_owned());
    }
    let relative = Path::new(path);
    let mut clean = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) if part.eq_ignore_ascii_case(".git") => {
                return Err("Files inside .git cannot be changed.".to_owned());
            }
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err("The path must stay inside the task.".to_owned()),
        }
    }
    Ok(clean)
}

pub(crate) fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    root.canonicalize().map_err(to_text)
}

/// An existing folder of the task, resolved through links; empty means the task's root.
pub(crate) fn resolve_dir(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let root = canonical_root(root)?;
    let dir = root
        .join(clean_relative(relative)?)
        .canonicalize()
        .map_err(|_| "The target folder does not exist.".to_owned())?;
    if !dir.starts_with(&root) {
        return Err("The target folder is outside the task.".to_owned());
    }
    if !dir.is_dir() {
        return Err("The target is not a folder.".to_owned());
    }
    Ok(dir)
}

fn source_name(source: &Path) -> Result<std::ffi::OsString, String> {
    source
        .file_name()
        .map(ToOwned::to_owned)
        .ok_or_else(|| "The dropped path has no name.".to_owned())
}

fn checked_sources(sources: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    if sources.is_empty() || sources.len() > MAX_DROPPED_SOURCES {
        return Err(format!("Drop between 1 and {MAX_DROPPED_SOURCES} files."));
    }
    let mut checked: Vec<PathBuf> = Vec::new();
    for source in sources {
        if !source.is_absolute() {
            return Err("The dropped path is not absolute.".to_owned());
        }
        let name = source_name(source)?;
        if checked
            .iter()
            .any(|other| other.file_name() == Some(name.as_os_str()))
        {
            return Err("Two dropped items have the same name.".to_owned());
        }
        checked.push(source.clone());
    }
    Ok(checked)
}

fn relative_text(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(to_text)?;
    let parts = relative
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "The path is not valid UTF-8.".to_owned())?;
    Ok(parts.join("/"))
}

fn move_inside(root: &Path, from: &str, to_dir: &str) -> Result<String, String> {
    let root = canonical_root(root)?;
    let relative = clean_relative(from)?;
    let name = relative
        .file_name()
        .ok_or_else(|| "Choose a file or folder to move.".to_owned())?
        .to_owned();
    let source = root.join(&relative);
    let kind =
        fs::symlink_metadata(&source).map_err(|_| "The item no longer exists.".to_owned())?;
    // A link moves as itself, but only if it and the folder holding it stay in the task.
    let parent = source
        .parent()
        .ok_or_else(|| "The item has no folder.".to_owned())?
        .canonicalize()
        .map_err(to_text)?;
    if !parent.starts_with(&root) {
        return Err("The item is outside the task.".to_owned());
    }
    if kind.file_type().is_symlink() {
        let target = source
            .canonicalize()
            .map_err(|_| "The link points outside the task.".to_owned())?;
        if !target.starts_with(&root) {
            return Err("The link points outside the task.".to_owned());
        }
    }
    let real_source = parent.join(&name);

    let target_dir = resolve_dir(&root, to_dir)?;
    if kind.is_dir() && !kind.file_type().is_symlink() && target_dir.starts_with(&real_source) {
        return Err("A folder cannot be moved into itself.".to_owned());
    }
    if target_dir == parent {
        return Err("The item is already in that folder.".to_owned());
    }
    let destination = target_dir.join(&name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(format!(
            "\"{}\" already exists in the target folder.",
            name.to_string_lossy()
        ));
    }
    fs::rename(&real_source, &destination).map_err(to_text)?;
    relative_text(&root, &destination)
}

fn copy_inside(
    root: &Path,
    sources: &[PathBuf],
    to_dir: &str,
    overwrite: bool,
) -> Result<Vec<String>, String> {
    let canonical = canonical_root(root)?;
    let target_dir = resolve_dir(&canonical, to_dir)?;
    let sources = checked_sources(sources)?;
    let mut plan = Vec::new();
    let mut budget = MAX_COPY_ENTRIES;
    for source in &sources {
        let real = source
            .canonicalize()
            .map_err(|_| "A dropped item no longer exists.".to_owned())?;
        let kind = fs::symlink_metadata(source).map_err(to_text)?;
        if kind.file_type().is_symlink() {
            return Err("Links cannot be copied.".to_owned());
        }
        if kind.is_dir() && target_dir.starts_with(&real) {
            return Err("A folder cannot be copied into itself.".to_owned());
        }
        let destination = target_dir.join(source_name(source)?);
        if destination == real {
            return Err("The item is already in that folder.".to_owned());
        }
        match fs::symlink_metadata(&destination) {
            Ok(_) if !overwrite => {
                return Err(format!(
                    "\"{}\" already exists in the target folder.",
                    source_name(source)?.to_string_lossy()
                ));
            }
            Ok(existing)
                if existing.is_dir() || existing.file_type().is_symlink() || kind.is_dir() =>
            {
                return Err(format!(
                    "\"{}\" already exists and is not a file that can be replaced.",
                    source_name(source)?.to_string_lossy()
                ));
            }
            _ => {}
        }
        plan_copy(&real, &destination, &mut budget, &mut plan)?;
    }
    for entry in &plan {
        match entry {
            Copy::Dir(path) => fs::create_dir_all(path).map_err(to_text)?,
            Copy::File(from, to) => {
                fs::copy(from, to).map_err(to_text)?;
            }
        }
    }
    sources
        .iter()
        .map(|source| relative_text(&canonical, &target_dir.join(source_name(source)?)))
        .collect()
}

enum Copy {
    Dir(PathBuf),
    File(PathBuf, PathBuf),
}

/// Lists what copying `from` to `to` creates, checking every entry before anything is written.
fn plan_copy(
    from: &Path,
    to: &Path,
    budget: &mut usize,
    plan: &mut Vec<Copy>,
) -> Result<(), String> {
    *budget = budget
        .checked_sub(1)
        .ok_or_else(|| format!("Copying more than {MAX_COPY_ENTRIES} items is not supported."))?;
    let kind = fs::symlink_metadata(from).map_err(to_text)?;
    if kind.file_type().is_symlink() {
        return Err("Links cannot be copied.".to_owned());
    }
    if kind.is_dir() {
        if from
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(".git"))
        {
            return Err("A .git folder cannot be copied.".to_owned());
        }
        plan.push(Copy::Dir(to.to_path_buf()));
        for entry in fs::read_dir(from).map_err(to_text)? {
            let entry = entry.map_err(to_text)?;
            plan_copy(&entry.path(), &to.join(entry.file_name()), budget, plan)?;
        }
    } else if kind.is_file() {
        plan.push(Copy::File(from.to_path_buf(), to.to_path_buf()));
    } else {
        return Err("Only files and folders can be copied.".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{copy_inside, move_inside};
    use std::fs;
    use std::path::Path;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        fs::write(path, text).expect("file");
    }

    fn task() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        write(&dir.path().join("a.txt"), "a");
        write(&dir.path().join("src/b.txt"), "b");
        write(&dir.path().join("src/deep/c.txt"), "c");
        fs::create_dir_all(dir.path().join("docs")).expect("docs");
        dir
    }

    #[test]
    fn moves_a_file_and_a_folder_by_rename() {
        let dir = task();
        assert_eq!(
            move_inside(dir.path(), "a.txt", "docs").as_deref(),
            Ok("docs/a.txt")
        );
        assert!(dir.path().join("docs/a.txt").is_file() && !dir.path().join("a.txt").exists());
        assert_eq!(
            move_inside(dir.path(), "src", "docs").as_deref(),
            Ok("docs/src")
        );
        assert!(dir.path().join("docs/src/deep/c.txt").is_file());
        assert_eq!(
            move_inside(dir.path(), "docs/a.txt", "").as_deref(),
            Ok("a.txt")
        );
    }

    #[test]
    fn refuses_a_folder_into_itself_and_a_clash() {
        let dir = task();
        assert!(move_inside(dir.path(), "src", "src/deep").is_err());
        assert!(move_inside(dir.path(), "src", "src").is_err());
        write(&dir.path().join("docs/a.txt"), "other");
        assert!(move_inside(dir.path(), "a.txt", "docs").is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("docs/a.txt")).expect("kept"),
            "other"
        );
        assert!(dir.path().join("a.txt").is_file());
    }

    #[test]
    fn refuses_paths_that_leave_the_task_or_touch_git() {
        let dir = task();
        assert!(move_inside(dir.path(), "../x.txt", "docs").is_err());
        assert!(move_inside(dir.path(), "a.txt", "..").is_err());
        assert!(move_inside(dir.path(), "a.txt", "src/../..").is_err());
        assert!(move_inside(dir.path(), "/etc/passwd", "docs").is_err());
        fs::create_dir_all(dir.path().join(".git")).expect("git");
        assert!(move_inside(dir.path(), "a.txt", ".git").is_err());
        assert!(move_inside(dir.path(), ".git", "docs").is_err());
        assert!(move_inside(dir.path(), "missing.txt", "docs").is_err());
        assert!(move_inside(dir.path(), "a.txt", "a.txt").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_links_that_leave_the_task() {
        let dir = task();
        let outside = tempfile::tempdir().expect("outside");
        write(&outside.path().join("secret.txt"), "s");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).expect("dir link");
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), dir.path().join("secret"))
            .expect("file link");
        assert!(move_inside(dir.path(), "a.txt", "escape").is_err());
        assert!(move_inside(dir.path(), "escape/secret.txt", "docs").is_err());
        assert!(move_inside(dir.path(), "secret", "docs").is_err());
        assert!(outside.path().join("secret.txt").is_file());
        std::os::unix::fs::symlink(dir.path().join("a.txt"), dir.path().join("inner"))
            .expect("link");
        assert_eq!(
            move_inside(dir.path(), "inner", "docs").as_deref(),
            Ok("docs/inner")
        );
    }

    #[test]
    fn copies_files_and_folders_from_outside() {
        let dir = task();
        let outside = tempfile::tempdir().expect("outside");
        write(&outside.path().join("one.txt"), "1");
        write(&outside.path().join("pics/two.png"), "2");
        let sources = [outside.path().join("one.txt"), outside.path().join("pics")];
        let copied = copy_inside(dir.path(), &sources, "docs", false).expect("copy");
        assert_eq!(copied, ["docs/one.txt", "docs/pics"]);
        assert_eq!(
            fs::read_to_string(dir.path().join("docs/pics/two.png")).expect("nested"),
            "2"
        );
        assert!(outside.path().join("one.txt").is_file());
    }

    #[test]
    fn copy_replaces_a_file_only_when_asked() {
        let dir = task();
        let outside = tempfile::tempdir().expect("outside");
        write(&outside.path().join("a.txt"), "new");
        let sources = [outside.path().join("a.txt")];
        assert!(copy_inside(dir.path(), &sources, "", false).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).expect("kept"),
            "a"
        );
        copy_inside(dir.path(), &sources, "", true).expect("overwrite");
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).expect("replaced"),
            "new"
        );
    }

    #[test]
    fn copy_never_replaces_a_folder_or_copies_the_task_into_itself() {
        let dir = task();
        let outside = tempfile::tempdir().expect("outside");
        write(&outside.path().join("docs/x.txt"), "x");
        assert!(copy_inside(dir.path(), &[outside.path().join("docs")], "", true).is_err());
        assert!(copy_inside(dir.path(), &[dir.path().join("src")], "src/deep", false).is_err());
        assert!(copy_inside(
            dir.path(),
            &[outside.path().join("docs/x.txt")],
            "../x",
            false
        )
        .is_err());
    }
}
