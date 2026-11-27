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
/// Files up to this size are hashed on every re-check: two writes of the same length within one
/// tick of the file system's clock leave size and time alike, and hashing this much is cheap.
const ALWAYS_HASH_BYTES: u64 = 64 * 1024;

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
        hash: crate::sha256::hex(&Sha256::digest(bytes)),
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

/// The start of a file the editor cannot edit, shown read-only.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHead {
    /// Up to the first 2 MiB as UTF-8, line ends as they are on disk, without a byte order mark.
    text: String,
    /// The file is longer than `text`.
    truncated: bool,
    size: u64,
}

/// Reads the start of a text file for a read-only view: one too large to edit, or one whose lines
/// do not all end the same way. A cut that falls inside a character ends before it.
///
/// # Errors
/// Refuses a path outside the task, and a file that is not UTF-8 text (`not_text`).
pub fn read_head(root: &Path, path: &str) -> Result<FileHead, String> {
    use std::io::Read;

    let file = resolve_preview_path(root, path)?;
    let size = fs::metadata(&file).map_err(to_text)?.len();
    let mut bytes = Vec::new();
    fs::File::open(&file)
        .and_then(|opened| opened.take(EDIT_MAX_BYTES).read_to_end(&mut bytes))
        .map_err(to_text)?;
    let truncated = size > bytes.len() as u64;
    let body = bytes.strip_prefix(BOM).unwrap_or(&bytes);
    let not_text = || refuse("not_text", "The file is not UTF-8 text.");
    if body.contains(&0) {
        return Err(not_text());
    }
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        // Only a character cut in two at the end of a truncated read is allowed.
        Err(error) if truncated && error.error_len().is_none() => {
            std::str::from_utf8(&body[..error.valid_up_to()]).map_err(|_| not_text())?
        }
        Err(_) => return Err(not_text()),
    };
    Ok(FileHead {
        text: text.to_owned(),
        truncated,
        size,
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

/// What a re-check found for an open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCheckStatus {
    Unchanged,
    Changed,
    Missing,
}

/// The result of re-checking a file against the version an editor last read.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCheck {
    status: FileCheckStatus,
    /// The file's current version; absent when it is missing.
    version: Option<FileVersion>,
}

/// Tells whether a file still has the version an editor last read.
///
/// Size and modified time are compared first. The content is read and hashed when either differs,
/// when the file is at most 64 KiB, or when `verify` asks for it: a same-length edit within one
/// tick of the file system's clock keeps both alike. Larger files are hashed only on `verify`, which
/// the window sends now and then, so a check stays cheap. A file whose time changed but whose
/// content did not is `Unchanged`, with the new time in `version` so the next check is cheap again.
///
/// # Errors
/// Refuses a path outside the task, and a file over 2 MiB (`too_large`).
pub fn check_editable(
    root: &Path,
    path: &str,
    expected: &FileVersion,
    verify: bool,
) -> Result<FileCheck, String> {
    let file = match resolve_preview_path(root, path) {
        Ok(file) => file,
        Err(failure) => {
            let plain = Path::new(path)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)));
            if plain && !path.is_empty() && !root.join(path).exists() {
                return Ok(FileCheck {
                    status: FileCheckStatus::Missing,
                    version: None,
                });
            }
            return Err(failure);
        }
    };
    let size = fs::metadata(&file).map_err(to_text)?.len();
    let modified = modified_ms(&file);
    if size == expected.size
        && modified == expected.modified_ms
        && size > ALWAYS_HASH_BYTES
        && !verify
    {
        return Ok(FileCheck {
            status: FileCheckStatus::Unchanged,
            version: Some(expected.clone()),
        });
    }
    let version = version_of(&read_limited(&file)?, modified);
    let status = if version.hash == expected.hash {
        FileCheckStatus::Unchanged
    } else {
        FileCheckStatus::Changed
    };
    Ok(FileCheck {
        status,
        version: Some(version),
    })
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

/// Reads the start of a task's file for a read-only view.
#[tauri::command]
pub async fn task_file_read_head(
    database: State<'_, Database>,
    id: String,
    path: String,
) -> Result<FileHead, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || read_head(&task.worktree_path, &path))
        .await
        .map_err(to_text)?
}

/// Re-checks an open file against the version its tab last read; `verify` always hashes it.
#[tauri::command]
pub async fn task_file_check(
    database: State<'_, Database>,
    id: String,
    path: String,
    expected: FileVersion,
    verify: Option<bool>,
) -> Result<FileCheck, String> {
    let task = TaskRecord::get(&*database.lock()?, &id).map_err(to_text)?;
    tauri::async_runtime::spawn_blocking(move || {
        check_editable(
            &task.worktree_path,
            &path,
            &expected,
            verify.unwrap_or(false),
        )
    })
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

    fn set_modified(file: &Path, time: std::time::SystemTime) {
        fs::OpenOptions::new()
            .write(true)
            .open(file)
            .expect("open")
            .set_modified(time)
            .expect("set time");
    }

    #[test]
    fn a_check_of_a_small_file_sees_a_same_size_same_time_edit_and_reports_what_changed() {
        let folder = folder_with("a.txt", b"aaaa");
        let file = folder.path().join("a.txt");
        let read = read_editable(folder.path(), "a.txt").expect("read");
        let then = fs::metadata(&file).expect("meta").modified().expect("time");

        // Same size and time, different bytes: a small file is hashed on every check, so it is seen.
        fs::write(&file, b"bbbb").expect("edit");
        set_modified(&file, then);
        let check = check_editable(folder.path(), "a.txt", &read.version, false).expect("check");
        assert_eq!(check.status, FileCheckStatus::Changed);
        fs::write(&file, b"aaaa").expect("restore");
        set_modified(&file, then);

        // A new time with the same bytes is unchanged, and hands back the new time.
        fs::write(&file, b"aaaa").expect("restore");
        set_modified(&file, then + std::time::Duration::from_secs(5));
        let check = check_editable(folder.path(), "a.txt", &read.version, false).expect("check");
        assert_eq!(check.status, FileCheckStatus::Unchanged);
        assert_ne!(
            check.version.expect("version").modified_ms,
            read.version.modified_ms
        );

        // Different bytes with a different time are a change.
        fs::write(&file, b"cccc").expect("edit");
        set_modified(&file, then + std::time::Duration::from_secs(9));
        let check = check_editable(folder.path(), "a.txt", &read.version, false).expect("check");
        assert_eq!(check.status, FileCheckStatus::Changed);

        fs::remove_file(&file).expect("remove");
        let check = check_editable(folder.path(), "a.txt", &read.version, false).expect("check");
        assert_eq!(check.status, FileCheckStatus::Missing);
        assert!(check_editable(folder.path(), "../a.txt", &read.version, false).is_err());
    }

    #[test]
    fn a_large_file_with_the_same_size_and_time_is_hashed_when_verify_asks() {
        let size = usize::try_from(ALWAYS_HASH_BYTES).expect("size") + 1;
        let folder = folder_with("big.txt", &vec![b'a'; size]);
        let file = folder.path().join("big.txt");
        let read = read_editable(folder.path(), "big.txt").expect("read");
        let then = fs::metadata(&file).expect("meta").modified().expect("time");

        fs::write(&file, vec![b'b'; size]).expect("edit");
        set_modified(&file, then);
        // A plain check of a large file compares only size and time, so it stays cheap.
        let check = check_editable(folder.path(), "big.txt", &read.version, false).expect("check");
        assert_eq!(check.status, FileCheckStatus::Unchanged);
        // The periodic verification hashes it and finds the edit.
        let check = check_editable(folder.path(), "big.txt", &read.version, true).expect("check");
        assert_eq!(check.status, FileCheckStatus::Changed);
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
    fn a_file_the_editor_refuses_can_still_be_read_from_the_start() {
        let folder = tempfile::tempdir().expect("folder");
        let put =
            |name: &str, bytes: &[u8]| fs::write(folder.path().join(name), bytes).expect("file");
        // A two-byte character straddles the 2 MiB cut.
        let mut big = vec![b'x'; 2 * 1024 * 1024 - 1];
        big.extend_from_slice("\u{e9}tail".as_bytes());
        put("big.txt", &big);
        put("mixed.txt", b"\xEF\xBB\xBFa\r\nb\nc");
        put("binary.bin", b"ab\0cd");
        put("latin1.txt", b"caf\xE9");

        let head = read_head(folder.path(), "big.txt").expect("big");
        assert!(head.truncated);
        assert_eq!(head.size, big.len() as u64);
        assert_eq!(head.text.len(), 2 * 1024 * 1024 - 1);

        let head = read_head(folder.path(), "mixed.txt").expect("mixed");
        assert_eq!((head.text.as_str(), head.truncated), ("a\r\nb\nc", false));

        for name in ["binary.bin", "latin1.txt"] {
            let error = read_head(folder.path(), name).map(|_| ()).expect_err(name);
            assert_eq!(code(&error), "not_text", "{name}");
        }
        assert!(read_head(folder.path(), "../big.txt").is_err());
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
