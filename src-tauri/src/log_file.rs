//! Size-bounded log file: the current file plus a few numbered archives.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_ARCHIVES: u32 = 3;
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// Appends to `<dir>/<base>`; before a write would pass the limit it shifts `base.1` .. `base.N`.
/// Each `write` call is kept whole in one file, so callers must pass whole records per call.
/// A failed rotation never fails a write: it keeps appending and tries again after a minute.
pub struct RotatingLog {
    dir: PathBuf,
    base_name: String,
    max_file_bytes: u64,
    max_files: u32,
    file: File,
    size: u64,
    retry_at: Option<Instant>,
}

fn open_append(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    // Lets another running instance rename or delete the file while this one holds it open.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ_WRITE_DELETE: u32 = 0x1 | 0x2 | 0x4;
        options.share_mode(FILE_SHARE_READ_WRITE_DELETE);
    }
    options.open(path)
}

impl RotatingLog {
    /// # Errors
    /// Fails only when the current log file cannot be opened; a failed startup rotation is deferred.
    pub fn open(
        dir: &Path,
        base_name: &str,
        max_file_bytes: u64,
        max_files: u32,
    ) -> io::Result<Self> {
        let file = open_append(&dir.join(base_name))?;
        let size = file.metadata()?.len();
        let mut log = Self {
            dir: dir.to_path_buf(),
            base_name: base_name.to_string(),
            max_file_bytes,
            max_files,
            file,
            size,
            retry_at: None,
        };
        if log.size > log.max_file_bytes {
            log.rotate_or_defer();
        }
        Ok(log)
    }

    fn path(&self, index: u32) -> PathBuf {
        if index == 0 {
            self.dir.join(&self.base_name)
        } else {
            self.dir.join(format!("{}.{index}", self.base_name))
        }
    }

    fn rotate(&mut self) -> io::Result<()> {
        if self.max_files == 0 {
            self.file.set_len(0)?;
            self.size = 0;
            return Ok(());
        }
        match fs::remove_file(self.path(self.max_files)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        for index in (0..self.max_files).rev() {
            match fs::rename(self.path(index), self.path(index + 1)) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
        }
        self.file = open_append(&self.path(0))?;
        self.size = 0;
        Ok(())
    }

    fn rotate_or_defer(&mut self) {
        self.retry_at = match self.rotate() {
            Ok(()) => None,
            Err(_) => Some(Instant::now() + RETRY_AFTER),
        };
    }
}

impl Write for RotatingLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let over = self.size > 0 && self.size + buf.len() as u64 > self.max_file_bytes;
        if over && self.retry_at.is_none_or(|at| Instant::now() >= at) {
            self.rotate_or_defer();
        }
        self.file.write_all(buf)?;
        self.size += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(dir: &Path, name: &str) -> String {
        fs::read_to_string(dir.join(name)).unwrap_or_default()
    }

    fn total_bytes(dir: &Path) -> u64 {
        fs::read_dir(dir)
            .expect("test io")
            .map(|entry| entry.expect("test io").metadata().expect("test io").len())
            .sum()
    }

    #[test]
    fn stays_bounded_and_keeps_records_whole() {
        let dir = tempfile::tempdir().expect("test io");
        let mut log = RotatingLog::open(dir.path(), "a.log", 100, 2).expect("test io");
        for number in 0..200 {
            log.write_all(format!("record {number:04} {}\n", "x".repeat(20)).as_bytes())
                .expect("test io");
            assert!(total_bytes(dir.path()) <= 3 * 100 + 40);
        }
        for name in ["a.log", "a.log.1", "a.log.2"] {
            for line in read(dir.path(), name).lines() {
                assert!(line.starts_with("record ") && line.ends_with('x'), "{line}");
            }
        }
        assert!(!dir.path().join("a.log.3").exists());
    }

    #[test]
    fn shifts_archives_in_order_and_drops_the_oldest() {
        let dir = tempfile::tempdir().expect("test io");
        let mut log = RotatingLog::open(dir.path(), "a.log", 10, 2).expect("test io");
        for record in ["aaaaaaaa\n", "bbbbbbbb\n", "cccccccc\n", "dddddddd\n"] {
            log.write_all(record.as_bytes()).expect("test io");
        }
        assert_eq!(read(dir.path(), "a.log"), "dddddddd\n");
        assert_eq!(read(dir.path(), "a.log.1"), "cccccccc\n");
        assert_eq!(read(dir.path(), "a.log.2"), "bbbbbbbb\n");
    }

    #[test]
    fn rotates_an_oversized_file_when_opened() {
        let dir = tempfile::tempdir().expect("test io");
        fs::write(dir.path().join("a.log"), "y".repeat(500)).expect("test io");
        let mut log = RotatingLog::open(dir.path(), "a.log", 100, 2).expect("test io");
        log.write_all(b"new\n").expect("test io");
        assert_eq!(read(dir.path(), "a.log"), "new\n");
        assert_eq!(read(dir.path(), "a.log.1"), "y".repeat(500));
    }

    #[test]
    fn failed_rotation_keeps_logging_and_retries_later() {
        let dir = tempfile::tempdir().expect("test io");
        let mut log = RotatingLog::open(dir.path(), "a.log", 20, 1).expect("test io");
        fs::create_dir(dir.path().join("a.log.1")).expect("test io");
        fs::write(dir.path().join("a.log.1").join("keep"), "k").expect("test io");
        for record in ["first record\n", "second record\n", "third record\n"] {
            log.write_all(record.as_bytes()).expect("test io");
        }
        assert_eq!(
            read(dir.path(), "a.log"),
            "first record\nsecond record\nthird record\n"
        );
        assert!(log.retry_at.is_some());

        fs::remove_dir_all(dir.path().join("a.log.1")).expect("test io");
        log.write_all(b"too soon\n").expect("test io");
        assert!(!dir.path().join("a.log.1").exists());
        log.retry_at = Some(Instant::now());
        log.write_all(b"after retry\n").expect("test io");
        assert_eq!(read(dir.path(), "a.log"), "after retry\n");
        assert!(read(dir.path(), "a.log.1").ends_with("too soon\n"));
        assert!(log.retry_at.is_none());
    }
}
