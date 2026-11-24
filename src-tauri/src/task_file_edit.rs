//! Reads and writes a task's text files for the editor.
//!
//! Files stay byte-faithful: the line ending (all `\n` or all `\r\n`) and a leading byte order mark
//! are reported when the file is read and put back when it is written, and nothing else about the
//! text is touched. A write succeeds only when the file still has the content the editor read, so
//! an agent's edit and a person's edit never silently overwrite each other. Every path must stay
//! inside the task's folder, by the same check the file preview uses.

use std::fs;
use std::path::Path;

use brachio_store::TaskRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::workspace::{resolve_preview_path, Database};

/// Largest file the editor opens or saves.
const EDIT_MAX_BYTES: u64 = 2 * 1024 * 1024;
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// The content a file had when it was read; `modified_ms` is for display, the hash decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    size: u64,
    modified_ms: i64,
    hash: String,
}

/// How a file ends its lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnding {
    Lf,
    Crlf,
}

/// A file as the editor shows it: text with `\n` line ends, and what is needed to write it back.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableFile {
    text: String,
    version: FileVersion,
    line_ending: LineEnding,
    bom: bool,
}

/// A refusal the window can tell apart: `<code>: <sentence>`.
fn refuse(code: &str, message: &str) -> String {
    format!("{code}: {message}")
}

fn to_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn version_of(bytes: &[u8], modified_ms: i64) -> FileVersion {
    FileVersion {
        size: bytes.len() as u64,
        modified_ms,
        hash: format!("{:x}", Sha256::digest(bytes)),
    }
}

fn modified_ms(file: &Path) -> i64 {
    fs::metadata(file)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn read_limited(file: &Path) -> Result<Vec<u8>, String> {
    let size = fs::metadata(file).map_err(to_text)?.len();
    if size > EDIT_MAX_BYTES {
        return Err(refuse("too_large", "The file is too large to edit here."));
    }
    fs::read(file).map_err(to_text)
}

/// Reads a text file for editing.
///
/// # Errors
/// Refuses, with a code, a path outside the task, a file over 2 MiB (`too_large`), one that is not
/// UTF-8 text (`not_text`), and one whose lines do not all end the same way (`mixed_endings`).
pub fn read_editable(root: &Path, path: &str) -> Result<EditableFile, String> {
    let file = resolve_preview_path(root, path)?;
    let bytes = read_limited(&file)?;
    let version = version_of(&bytes, modified_ms(&file));
    let (bom, body) = match bytes.strip_prefix(BOM) {
        Some(rest) => (true, rest),
        None => (false, bytes.as_slice()),
    };
    if body.contains(&0) {
        return Err(refuse("not_text", "The file is not text."));
    }
    let text =
        std::str::from_utf8(body).map_err(|_| refuse("not_text", "The file is not UTF-8 text."))?;
    let crlf = text.matches("\r\n").count();
    let lone_lf = text.matches('\n').count() - crlf;
    let lone_cr = text.matches('\r').count() - crlf;
    if lone_cr > 0 || (crlf > 0 && lone_lf > 0) {
        return Err(refuse(
            "mixed_endings",
            "The lines of the file do not all end the same way.",
        ));
    }
    let line_ending = if crlf > 0 {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    };
    Ok(EditableFile {
        text: text.replace("\r\n", "\n"),
        version,
        line_ending,
        bom,
    })
}

/// Why a save stopped, as the message the interface shows.
struct SaveFailure(String);

impl From<std::io::Error> for SaveFailure {
    fn from(error: std::io::Error) -> Self {
        Self(to_text(error))
    }
}

impl From<String> for SaveFailure {
    fn from(message: String) -> Self {
        Self(message)
    }
}

/// Writes `text` over a file that still has the content the editor read.
///
/// The new content goes to a temporary file in the same folder and replaces the old one in a
/// single rename (`brachio_fs::write_atomic_checked`), so a crash never leaves half a file.
///
/// # Errors
/// Refuses, with a code, a path outside the task, text over 2 MiB (`too_large`), text holding a
/// carriage return (`invalid_text`), and a file whose content is not the one read (`conflict`).
pub fn write_editable(
    root: &Path,
    path: &str,
    text: &str,
    expected: &FileVersion,
    line_ending: LineEnding,
    bom: bool,
) -> Result<FileVersion, String> {
    if text.contains('\r') {
        return Err(refuse(
            "invalid_text",
            "The text has a carriage return; line ends must be plain.",
        ));
    }
    let file = resolve_preview_path(root, path)?;
    let mut content = Vec::with_capacity(text.len() + 3);
    if bom {
        content.extend_from_slice(BOM);
    }
    match line_ending {
        LineEnding::Lf => content.extend_from_slice(text.as_bytes()),
        LineEnding::Crlf => content.extend_from_slice(text.replace('\n', "\r\n").as_bytes()),
    }
    if content.len() as u64 > EDIT_MAX_BYTES {
        return Err(refuse("too_large", "The text is too large to save here."));
    }
    let conflict = || {
        refuse(
            "conflict",
            "The file changed since it was opened. Nothing was written.",
        )
    };
    let current = read_limited(&file)?;
    if version_of(&current, 0).hash != expected.hash {
        return Err(conflict());
    }
    brachio_fs::write_atomic_checked(&file, &content, || -> Result<(), SaveFailure> {
        // The file may have changed while the new content was written.
        if version_of(&read_limited(&file)?, 0).hash != expected.hash {
            return Err(SaveFailure(conflict()));
        }
        Ok(())
    })
    .map_err(|failure| failure.0)?;
    Ok(version_of(&content, modified_ms(&file)))
}

/// Reads a task's text file for the editor.
#[tauri::command]
pub async fn task_file_read(
    database: State<'_, Database>,
    id: String,
    path: String,
) -> Result<EditableFile, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || read_editable(&task.worktree_path, &path))
        .await
        .map_err(to_text)?
}

/// Saves the editor's text over a task's file, unless the file changed since it was read.
#[tauri::command]
pub async fn task_file_write(
    database: State<'_, Database>,
    id: String,
    path: String,
    text: String,
    expected: FileVersion,
    line_ending: LineEnding,
    bom: bool,
) -> Result<FileVersion, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        write_editable(
            &task.worktree_path,
            &path,
            &text,
            &expected,
            line_ending,
            bom,
        )
    })
    .await
    .map_err(to_text)?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_with(name: &str, bytes: &[u8]) -> tempfile::TempDir {
        let folder = tempfile::tempdir().expect("folder");
        fs::write(folder.path().join(name), bytes).expect("file");
        folder
    }

    fn code(error: &str) -> &str {
        error.split(':').next().unwrap_or(error)
    }

    #[test]
    fn a_file_is_shown_with_plain_line_ends_and_written_back_the_way_it_was() {
        let folder = folder_with("a.txt", b"one\r\ntwo\r\n");
        let read = read_editable(folder.path(), "a.txt").expect("read");
        assert_eq!(read.text, "one\ntwo\n");
        assert_eq!(read.line_ending, LineEnding::Crlf);
        assert!(!read.bom);
        write_editable(
            folder.path(),
            "a.txt",
            "one\nTWO\nthree",
            &read.version,
            read.line_ending,
            read.bom,
        )
        .expect("write");
        assert_eq!(
            fs::read(folder.path().join("a.txt")).expect("bytes"),
            b"one\r\nTWO\r\nthree"
        );
    }

    #[test]
    fn an_lf_file_with_a_byte_order_mark_and_no_final_newline_stays_that_way() {
        let folder = folder_with("a.md", b"\xEF\xBB\xBF# Title\nbody");
        let read = read_editable(folder.path(), "a.md").expect("read");
        assert_eq!(
            (read.text.as_str(), read.line_ending, read.bom),
            ("# Title\nbody", LineEnding::Lf, true)
        );
        write_editable(
            folder.path(),
            "a.md",
            "# Title\nbody!",
            &read.version,
            read.line_ending,
            read.bom,
        )
        .expect("write");
        assert_eq!(
            fs::read(folder.path().join("a.md")).expect("bytes"),
            b"\xEF\xBB\xBF# Title\nbody!"
        );
    }

    #[test]
    fn a_write_after_someone_else_changed_the_file_is_a_conflict_and_changes_nothing() {
        let folder = folder_with("a.txt", b"mine\n");
        let read = read_editable(folder.path(), "a.txt").expect("read");
        fs::write(folder.path().join("a.txt"), b"the agent's\n").expect("agent edit");
        let error = write_editable(
            folder.path(),
            "a.txt",
            "mine!\n",
            &read.version,
            read.line_ending,
            read.bom,
        )
        .expect_err("conflict");
        assert_eq!(code(&error), "conflict");
        assert_eq!(
            fs::read(folder.path().join("a.txt")).expect("bytes"),
            b"the agent's\n"
        );
        let leftovers: Vec<_> = fs::read_dir(folder.path())
            .expect("dir")
            .flatten()
            .collect();
        assert_eq!(leftovers.len(), 1, "no temporary file stays behind");
    }

    #[test]
    fn touching_the_file_without_changing_it_is_not_a_conflict() {
        let folder = folder_with("a.txt", b"same\n");
        let read = read_editable(folder.path(), "a.txt").expect("read");
        fs::write(folder.path().join("a.txt"), b"same\n").expect("rewrite with the same content");
        let saved = write_editable(
            folder.path(),
            "a.txt",
            "same too\n",
            &read.version,
            read.line_ending,
            read.bom,
        )
        .expect("write");
        assert_eq!(saved.size, 9);
        let again = read_editable(folder.path(), "a.txt").expect("read again");
        assert_eq!(again.version.hash, saved.hash);
    }

    #[test]
    fn files_the_editor_cannot_keep_faithful_are_refused_with_a_code() {
        let folder = tempfile::tempdir().expect("folder");
        let put =
            |name: &str, bytes: &[u8]| fs::write(folder.path().join(name), bytes).expect("file");
        put("binary.bin", b"ab\0cd");
        put("latin1.txt", b"caf\xE9");
        put("mixed.txt", b"a\r\nb\nc\n");
        put("old-mac.txt", b"a\rb\r");
        put("big.txt", &vec![b'x'; 2 * 1024 * 1024 + 1]);
        for (name, wanted) in [
            ("binary.bin", "not_text"),
            ("latin1.txt", "not_text"),
            ("mixed.txt", "mixed_endings"),
            ("old-mac.txt", "mixed_endings"),
            ("big.txt", "too_large"),
        ] {
            let error = read_editable(folder.path(), name)
                .map(|_| ())
                .expect_err(name);
            assert_eq!(code(&error), wanted, "{name}: {error}");
        }
    }

    #[test]
    fn text_that_cannot_be_written_faithfully_is_refused() {
        let folder = folder_with("a.txt", b"x\n");
        let read = read_editable(folder.path(), "a.txt").expect("read");
        let carriage = write_editable(
            folder.path(),
            "a.txt",
            "a\rb",
            &read.version,
            read.line_ending,
            read.bom,
        )
        .expect_err("carriage return");
        assert_eq!(code(&carriage), "invalid_text");
        let big = "x".repeat(2 * 1024 * 1024 + 1);
        let large = write_editable(
            folder.path(),
            "a.txt",
            &big,
            &read.version,
            read.line_ending,
            read.bom,
        )
        .expect_err("too large");
        assert_eq!(code(&large), "too_large");
        assert_eq!(
            fs::read(folder.path().join("a.txt")).expect("bytes"),
            b"x\n"
        );
    }

    #[test]
    fn paths_outside_the_task_or_inside_git_are_refused() {
        let outer = tempfile::tempdir().expect("outer");
        fs::write(outer.path().join("secret.txt"), b"s").expect("secret");
        let task = outer.path().join("task");
        fs::create_dir_all(task.join(".git")).expect("git");
        fs::write(task.join(".git/config"), b"c").expect("config");
        fs::write(task.join("ok.txt"), b"ok").expect("file");
        let read = read_editable(&task, "ok.txt").expect("read");
        for path in [
            "../secret.txt",
            ".git/config",
            "",
            "/etc/hosts",
            "ok.txt/../../secret.txt",
            "missing.txt",
        ] {
            assert!(
                read_editable(&task, path).is_err(),
                "{path:?} must be refused to read"
            );
            assert!(
                write_editable(&task, path, "x", &read.version, LineEnding::Lf, false).is_err(),
                "{path:?} must be refused to write"
            );
        }
        assert_eq!(
            fs::read(outer.path().join("secret.txt")).expect("bytes"),
            b"s"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leaves_the_task_is_not_followed_and_permissions_are_kept() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let outer = tempfile::tempdir().expect("outer");
        fs::write(outer.path().join("secret.txt"), b"s").expect("secret");
        let task = outer.path().join("task");
        fs::create_dir_all(&task).expect("task");
        symlink(outer.path().join("secret.txt"), task.join("link.txt")).expect("link");
        assert!(read_editable(&task, "link.txt").is_err());

        fs::write(task.join("run.sh"), b"#!/bin/sh\n").expect("script");
        fs::set_permissions(task.join("run.sh"), fs::Permissions::from_mode(0o750)).expect("mode");
        let read = read_editable(&task, "run.sh").expect("read");
        write_editable(
            &task,
            "run.sh",
            "#!/bin/sh\necho hi\n",
            &read.version,
            read.line_ending,
            read.bom,
        )
        .expect("write");
        let mode = fs::metadata(task.join("run.sh"))
            .expect("meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o750);
    }

    #[cfg(windows)]
    #[test]
    fn a_save_succeeds_while_another_process_briefly_holds_the_file_open() {
        use std::os::windows::fs::OpenOptionsExt;

        let folder = folder_with(
            "a.txt", b"one
",
        );
        let read = read_editable(folder.path(), "a.txt").expect("read");
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(folder.path().join("a.txt"))
            .expect("hold");
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            drop(held);
        });
        write_editable(
            folder.path(),
            "a.txt",
            "two
",
            &read.version,
            LineEnding::Lf,
            false,
        )
        .expect("write");
        release.join().expect("release");
        assert_eq!(
            fs::read(folder.path().join("a.txt")).expect("read"),
            b"two
"
        );
    }
}
