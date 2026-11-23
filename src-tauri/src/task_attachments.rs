//! Saves images a person attaches to a prompt into the task's worktree, so the agent can read them by path.

use std::fs;
use std::path::{Path, PathBuf};

use brachio_store::TaskRecord;
use tauri::State;

use crate::workspace::Database;

const ATTACHMENT_DIR: &str = ".brachio/attachments";
const MAX_IMAGES: usize = 4;
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Copies images into `.brachio/attachments/` of the task and returns their worktree-relative paths.
/// The folder ignores itself, so the images never show up as changes.
#[tauri::command]
pub async fn task_attach_images(
    database: State<'_, Database>,
    id: String,
    sources: Vec<PathBuf>,
) -> Result<Vec<String>, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || save_images(&task.worktree_path, &sources))
        .await
        .map_err(to_text)?
}

fn save_images(root: &Path, sources: &[PathBuf]) -> Result<Vec<String>, String> {
    if sources.is_empty() || sources.len() > MAX_IMAGES {
        return Err(format!("Attach between 1 and {MAX_IMAGES} images."));
    }
    let mut checked = Vec::new();
    for source in sources {
        let extension = source
            .extension()
            .and_then(|text| text.to_str())
            .map(str::to_ascii_lowercase)
            .filter(|text| IMAGE_EXTENSIONS.contains(&text.as_str()))
            .ok_or_else(|| "Only PNG, JPEG, GIF and WebP images can be attached.".to_owned())?;
        let kind = fs::symlink_metadata(source).map_err(|_| "An image no longer exists.".to_owned())?;
        if !kind.is_file() {
            return Err("Only image files can be attached.".to_owned());
        }
        if kind.len() > MAX_IMAGE_BYTES {
            return Err(format!("An image is larger than {} MB.", MAX_IMAGE_BYTES / 1024 / 1024));
        }
        checked.push((source, extension));
    }
    let root = root.canonicalize().map_err(to_text)?;
    let folder = root.join(ATTACHMENT_DIR);
    fs::create_dir_all(&folder).map_err(to_text)?;
    let ignore = root.join(".brachio").join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, "*\n").map_err(to_text)?;
    }
    let mut saved = Vec::new();
    for (source, extension) in checked {
        let stem: String = source
            .file_stem()
            .and_then(|text| text.to_str())
            .unwrap_or("image")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .take(40)
            .collect();
        let mut name = format!("{stem}.{extension}");
        let mut counter = 1;
        while folder.join(&name).exists() {
            counter += 1;
            name = format!("{stem}-{counter}.{extension}");
        }
        fs::copy(source, folder.join(&name)).map_err(to_text)?;
        saved.push(format!("{ATTACHMENT_DIR}/{name}"));
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::save_images;
    use std::fs;

    #[test]
    fn copies_images_into_a_self_ignoring_folder_without_overwriting() {
        let task = tempfile::tempdir().expect("task");
        let elsewhere = tempfile::tempdir().expect("source");
        let shot = elsewhere.path().join("My shot.PNG");
        fs::write(&shot, b"png").expect("image");

        let first = save_images(task.path(), &[shot.clone()]).expect("first");
        let second = save_images(task.path(), &[shot]).expect("second");

        assert_eq!(first, [".brachio/attachments/My-shot.png"]);
        assert_eq!(second, [".brachio/attachments/My-shot-2.png"]);
        assert_eq!(fs::read_to_string(task.path().join(".brachio/.gitignore")).expect("ignore"), "*\n");
    }

    #[test]
    fn rejects_other_files_and_oversized_images() {
        let task = tempfile::tempdir().expect("task");
        let text = task.path().join("notes.txt");
        fs::write(&text, "x").expect("file");
        assert!(save_images(task.path(), &[text]).is_err());

        let big = task.path().join("big.png");
        fs::File::create(&big).expect("big").set_len(11 * 1024 * 1024).expect("size");
        assert!(save_images(task.path(), &[big]).is_err());
        assert!(save_images(task.path(), &[]).is_err());
    }
}
