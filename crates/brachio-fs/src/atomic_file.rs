//! Crash-safe writes and corrupt-file handling for files Brachio owns.
//!
//! A file is replaced in one step: write a scratch file in the same directory, flush it to disk,
//! then rename it over the target. A crash leaves the old file or the new one, never half of
//! either. A file that cannot be parsed is renamed to `<name>.corrupt-<ms>` so the next write
//! cannot overwrite the evidence.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// How often a rename is tried when another process briefly holds the target open (Windows).
const REPLACE_ATTEMPTS: u32 = 5;

/// Replaces `path` with `bytes` so that a reader or a crash sees the old or the new content only.
///
/// # Errors
///
/// Fails when the scratch file cannot be written or the rename fails. The target is left as it was
/// and the scratch file is removed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_scratch_then_replace(path, bytes, false, || Ok(()))
}

/// Like [`write_atomic`], for a file someone else owns: the new file keeps the permissions of the
/// one it replaces, and `before_replace` runs right before the rename. When it fails nothing is
/// replaced, the scratch file is removed and its error is returned.
///
/// # Errors
///
/// Fails like [`write_atomic`], or with the error `before_replace` returned.
pub fn write_atomic_checked<E: From<io::Error>>(
    path: &Path,
    bytes: &[u8],
    before_replace: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    write_scratch_then_replace(path, bytes, true, before_replace)
}

fn write_scratch_then_replace<E: From<io::Error>>(
    path: &Path,
    bytes: &[u8],
    keep_permissions: bool,
    before_replace: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    let directory = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = file_name(path)?;
    let counter = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let scratch = directory.join(format!(".{name}.{}-{counter}.tmp", std::process::id()));

    let result = (|| -> Result<(), E> {
        write_scratch(&scratch, bytes)?;
        if keep_permissions {
            match fs::metadata(path) {
                Ok(existing) => fs::set_permissions(&scratch, existing.permissions())?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        before_replace()?;
        Ok(replace(&scratch, path)?)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&scratch);
        return result;
    }
    sync_directory(directory);
    Ok(())
}

fn file_name(path: &Path) -> io::Result<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))
}

fn write_scratch(scratch: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::options().write(true).create_new(true).open(scratch)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Renames over an existing file. `std` replaces on every platform; Windows can still refuse
/// while a reader has the target open, so that case is tried again.
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Err(error)
                if cfg!(windows)
                    && error.kind() == io::ErrorKind::PermissionDenied
                    && attempt < REPLACE_ATTEMPTS =>
            {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

/// Makes the rename itself durable. Directories cannot be opened for sync on Windows.
fn sync_directory(directory: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = File::open(directory) {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = directory;
}

/// Moves an unreadable file out of the way and returns where it went.
///
/// # Errors
///
/// Fails when the rename fails.
pub fn quarantine_corrupt(path: &Path) -> io::Result<PathBuf> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    let name = file_name(path)?;
    let mut target = path.with_file_name(format!("{name}.corrupt-{millis}"));
    let mut suffix = 1;
    while target.exists() {
        target = path.with_file_name(format!("{name}.corrupt-{millis}-{suffix}"));
        suffix += 1;
    }
    fs::rename(path, &target)?;
    Ok(target)
}

/// What reading an owned file found.
#[derive(Debug, PartialEq, Eq)]
pub enum Stored<T> {
    /// There is no file yet; use defaults.
    Missing,
    /// The file parsed.
    Found(T),
    /// The file did not parse and was moved here; use defaults and log the path.
    Quarantined(PathBuf),
}

/// Reads and parses an owned file. A file `parse` rejects is quarantined, never deleted or
/// silently overwritten, so the caller can use defaults and log the returned path.
///
/// # Errors
///
/// Fails on I/O errors other than a missing file, including a failed quarantine.
pub fn read_or_quarantine<T>(
    path: &Path,
    parse: impl FnOnce(&[u8]) -> Option<T>,
) -> io::Result<Stored<T>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Stored::Missing),
        Err(error) => return Err(error),
    };
    match parse(&bytes) {
        Some(value) => Ok(Stored::Found(value)),
        None => quarantine_corrupt(path).map(Stored::Quarantined),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn replaces_content_and_leaves_no_scratch_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("state.json");
        write_atomic(&path, b"first").expect("create");
        write_atomic(&path, b"second").expect("replace");
        assert_eq!(fs::read(&path).expect("read"), b"second");
        assert_eq!(entries(dir.path()), ["state.json"]);
    }

    #[test]
    fn a_failed_replace_keeps_the_target_and_removes_the_scratch_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("state.json");
        // A directory cannot be replaced by a file, which forces the rename to fail.
        fs::create_dir(&path).expect("block target");
        fs::write(path.join("keep"), b"x").expect("fill");
        assert!(write_atomic(&path, b"new").is_err());
        assert_eq!(entries(dir.path()), ["state.json"]);
        assert!(path.join("keep").exists());
    }

    #[test]
    fn fails_without_creating_anything_when_the_folder_is_missing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("missing").join("state.json");
        assert!(write_atomic(&path, b"x").is_err());
        assert!(entries(dir.path()).is_empty());
    }

    #[test]
    fn a_leftover_scratch_file_does_not_affect_the_target() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("state.json");
        write_atomic(&path, b"good").expect("write");
        fs::write(dir.path().join(".state.json.1-0.tmp"), b"{incomplete").expect("leftover");
        write_atomic(&path, b"better").expect("write again");
        assert_eq!(fs::read(&path).expect("read"), b"better");
    }

    #[test]
    fn a_failing_check_keeps_the_target_and_removes_the_scratch_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.txt");
        fs::write(&path, b"old").expect("write");
        let result: Result<(), io::Error> =
            write_atomic_checked(&path, b"new", || Err(io::Error::other("changed")));
        assert!(result.is_err());
        assert_eq!(fs::read(&path).expect("read"), b"old");
        assert_eq!(entries(dir.path()), ["a.txt"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_checked_write_keeps_the_permissions_of_the_file_it_replaces() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.txt");
        fs::write(&path, b"old").expect("write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set");
        write_atomic_checked::<io::Error>(&path, b"new", || Ok(())).expect("write");
        assert_eq!(fs::read(&path).expect("read"), b"new");
        let mode = fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o640);
    }

    #[cfg(windows)]
    #[test]
    fn a_target_another_process_briefly_holds_open_is_still_replaced() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.txt");
        fs::write(&path, b"old").expect("write");
        // Without FILE_SHARE_DELETE a rename over the file is refused while it is open.
        let held = File::options()
            .read(true)
            .share_mode(1)
            .open(&path)
            .expect("hold");
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            drop(held);
        });
        write_atomic_checked::<io::Error>(&path, b"new", || Ok(())).expect("write");
        release.join().expect("release");
        assert_eq!(fs::read(&path).expect("read"), b"new");
    }

    fn parse_number(bytes: &[u8]) -> Option<u32> {
        std::str::from_utf8(bytes).ok()?.trim().parse().ok()
    }

    #[test]
    fn reads_missing_valid_and_corrupt_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("count.txt");
        assert_eq!(
            read_or_quarantine(&path, parse_number).expect("missing"),
            Stored::Missing
        );

        write_atomic(&path, b"42").expect("write");
        assert_eq!(
            read_or_quarantine(&path, parse_number).expect("valid"),
            Stored::Found(42)
        );

        fs::write(&path, b"{not a number").expect("corrupt");
        let Stored::Quarantined(moved) = read_or_quarantine(&path, parse_number).expect("corrupt")
        else {
            panic!("corrupt file must be quarantined");
        };
        assert!(!path.exists());
        assert_eq!(fs::read(&moved).expect("kept"), b"{not a number");
        let name = moved
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with("count.txt.corrupt-"), "{name}");
    }

    #[test]
    fn quarantining_twice_in_one_millisecond_keeps_both_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.json");
        let mut moved = Vec::new();
        for text in ["one", "two"] {
            fs::write(&path, text).expect("write");
            moved.push(quarantine_corrupt(&path).expect("quarantine"));
        }
        assert_ne!(moved[0], moved[1]);
        assert_eq!(fs::read(&moved[0]).expect("first"), b"one");
        assert_eq!(fs::read(&moved[1]).expect("second"), b"two");
    }
}
