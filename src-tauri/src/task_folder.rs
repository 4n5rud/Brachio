//! Opens a task's worktree folder, or the app's log folder, in the system file manager.

use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::State;

use brachio_store::TaskRecord;

use crate::workspace::Database;

/// The task's folder as recorded when it was created, if it is still a folder on disk.
fn task_folder(database: &Database, id: &str) -> Result<PathBuf, String> {
    let task = TaskRecord::get(&*database.lock()?, id).map_err(|error| error.to_string())?;
    existing_folder(&task.worktree_path)
}

fn existing_folder(path: &Path) -> Result<PathBuf, String> {
    let folder = path
        .canonicalize()
        .map_err(|_| format!("{} no longer exists.", path.display()))?;
    if !folder.is_dir() {
        return Err(format!("{} is not a folder.", path.display()));
    }
    Ok(brachio_pty::without_verbatim_prefix(&folder))
}

/// Shows the task's worktree in Finder or Explorer, selected in its parent folder.
#[tauri::command]
pub async fn task_reveal(database: State<'_, Database>, id: String) -> Result<(), String> {
    let folder = task_folder(&database, &id)?;
    run(reveal_command(&folder)).await
}

/// Opens the task's worktree with the system's default handler for folders. Brachio has no
/// editor setting, so this is the file manager rather than a code editor.
#[tauri::command]
pub async fn task_open_folder(database: State<'_, Database>, id: String) -> Result<(), String> {
    let folder = task_folder(&database, &id)?;
    run(open_command(&folder)).await
}

/// Opens the folder that holds `brachio.log`.
#[tauri::command]
pub async fn open_log_folder() -> Result<(), String> {
    let folder = crate::log_file_path()
        .and_then(|file| file.parent().map(Path::to_path_buf))
        .ok_or_else(|| "The log folder is not known on this system.".to_owned())?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    run(open_command(&folder)).await
}

fn reveal_command(folder: &Path) -> Command {
    #[cfg(windows)]
    {
        let mut command = explorer();
        command.arg(format!("/select,{}", folder.display()));
        command
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg("-R").arg(folder);
        command
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        open_command(folder)
    }
}

fn open_command(folder: &Path) -> Command {
    #[cfg(windows)]
    {
        let mut command = explorer();
        command.arg(folder);
        command
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(folder);
        command
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(folder);
        command
    }
}

#[cfg(windows)]
fn explorer() -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("explorer");
    command.creation_flags(0x0800_0000);
    command
}

async fn run(mut command: Command) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Explorer exits with 1 even when it opened the window, so only a failure to start counts.
        let status = command.status().map_err(|error| error.to_string())?;
        if cfg!(windows) || status.success() {
            Ok(())
        } else {
            Err(format!("Could not open the folder ({status})."))
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::existing_folder;

    #[test]
    fn only_a_folder_that_exists_is_opened() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(existing_folder(dir.path()).is_ok());
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "").expect("write");
        assert!(existing_folder(&file).is_err());
        assert!(existing_folder(&dir.path().join("gone")).is_err());
    }
}
