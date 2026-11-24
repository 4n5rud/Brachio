//! Reads a task's image or PDF for the file tab.
//!
//! The path check is the one the editor's reads use, the size is limited, and the bytes must look
//! like the format the extension claims. The bytes go to the window as they are, without JSON.

use std::io::Read;
use std::path::Path;

use brachio_store::TaskRecord;
use tauri::State;

use crate::workspace::{resolve_preview_path, Database};

/// Largest image or PDF the file tab opens.
const MEDIA_MAX_BYTES: usize = 8 * 1024 * 1024;

fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn matches_format(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        "pdf" => bytes.starts_with(b"%PDF-"),
        "svg" => {
            let head = &bytes[..bytes.len().min(2048)];
            std::str::from_utf8(head).is_ok_and(|text| text.contains("<svg"))
        }
        _ => false,
    }
}

/// Reads a supported media file of a task.
///
/// # Errors
/// Refuses a path outside the task, a file over 8 MiB, and one whose bytes do not match its
/// extension (`png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`, `pdf`).
pub fn read_media(root: &Path, path: &str) -> Result<Vec<u8>, String> {
    let file = resolve_preview_path(root, path)?;
    let extension = file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let mut bytes = Vec::new();
    std::fs::File::open(&file)
        .map_err(to_text)?
        .take(MEDIA_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(to_text)?;
    if bytes.len() > MEDIA_MAX_BYTES {
        return Err("too_large: The file is larger than 8 MiB.".to_owned());
    }
    if !matches_format(&extension, &bytes) {
        return Err("not_media: The file is not a supported image or PDF.".to_owned());
    }
    Ok(bytes)
}

/// Reads a task's image or PDF.
#[tauri::command]
pub async fn task_file_media(
    database: State<'_, Database>,
    id: String,
    path: String,
) -> Result<tauri::ipc::Response, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || read_media(&task.worktree_path, &path))
        .await
        .map_err(to_text)?
        .map(tauri::ipc::Response::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(error: &str) -> &str {
        error.split(':').next().unwrap_or(error)
    }

    #[test]
    fn media_is_read_only_when_the_bytes_match_the_extension_and_stay_in_the_task() {
        let folder = tempfile::tempdir().expect("folder");
        let put = |name: &str, bytes: &[u8]| {
            std::fs::write(folder.path().join(name), bytes).expect("file")
        };
        put("a.png", b"\x89PNG\r\n\x1a\nrest");
        put("fake.png", b"not a png");
        put(
            "a.svg",
            b"<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        );
        put("page.svg", b"<html></html>");
        put("a.pdf", b"%PDF-1.7 body");
        put("notes.txt", b"%PDF-1.7 body");
        put(
            "big.gif",
            &[b'G', b'I', b'F', b'8', b'9', b'a']
                .iter()
                .copied()
                .chain(std::iter::repeat_n(0, MEDIA_MAX_BYTES))
                .collect::<Vec<u8>>(),
        );

        assert_eq!(read_media(folder.path(), "a.png").expect("png").len(), 12);
        assert!(read_media(folder.path(), "a.svg").is_ok());
        assert!(read_media(folder.path(), "a.pdf").is_ok());
        for name in ["fake.png", "page.svg", "notes.txt"] {
            assert_eq!(
                code(&read_media(folder.path(), name).expect_err(name)),
                "not_media"
            );
        }
        assert_eq!(
            code(&read_media(folder.path(), "big.gif").expect_err("big")),
            "too_large"
        );
        assert!(read_media(folder.path(), "../a.png").is_err());
    }
}
