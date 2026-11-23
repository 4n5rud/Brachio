//! Keeps the last known screen of every shell on disk, so a dead runtime can be recovered from.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use brachio_fs::{read_or_quarantine, write_atomic, Stored};
use serde::{Deserialize, Serialize};

/// What survives a runtime: enough to show the last screen and to start the shell again.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionRecord {
    pub(crate) key: String,
    pub(crate) cwd: PathBuf,
    /// The runtime that wrote this record, so a reader can tell it is not the current one.
    pub(crate) instance: String,
    pub(crate) pid: u32,
    pub(crate) rows: u16,
    pub(crate) cols: u16,
    pub(crate) ansi: String,
    pub(crate) cursor: (u16, u16),
    pub(crate) alternate_screen: bool,
    pub(crate) output_bytes: u64,
    pub(crate) exited: bool,
    pub(crate) exit_code: Option<u32>,
    pub(crate) saved_at_ms: u64,
}

/// A directory of one JSON file per shell key.
pub(crate) struct Journal {
    directory: PathBuf,
    #[cfg(test)]
    pub(crate) before_save: Option<Box<dyn Fn() + Send + Sync>>,
}

/// Records read at startup, and the files that could not be read.
pub(crate) struct Loaded {
    pub(crate) records: Vec<SessionRecord>,
    pub(crate) unreadable: Vec<PathBuf>,
}

impl Journal {
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        brachio_runtime::paths::ensure_private_directory(directory)?;
        Ok(Self {
            directory: directory.to_owned(),
            #[cfg(test)]
            before_save: None,
        })
    }

    /// Replace the record in one step, so a crash leaves the old or the new one, never half.
    pub(crate) fn save(&self, record: &SessionRecord) -> io::Result<()> {
        #[cfg(test)]
        if let Some(hook) = &self.before_save {
            hook();
        }
        let path = self.path(&record.key);
        write_atomic(
            &path,
            &serde_json::to_vec(record).map_err(io::Error::other)?,
        )
    }

    pub(crate) fn remove(&self, key: &str) -> io::Result<()> {
        match fs::remove_file(self.path(key)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    /// Read every record. A damaged file is renamed to `*.corrupt-<ms>`, so it affects no other shell.
    pub(crate) fn load(&self) -> io::Result<Loaded> {
        let mut loaded = Loaded {
            records: Vec::new(),
            unreadable: Vec::new(),
        };
        for entry in fs::read_dir(&self.directory)? {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let read = read_or_quarantine(&path, |bytes| {
                serde_json::from_slice::<SessionRecord>(bytes).ok()
            });
            match read {
                Ok(Stored::Found(record)) => loaded.records.push(record),
                Ok(Stored::Quarantined(moved_to)) => loaded.unreadable.push(moved_to),
                // Vanished since the listing, or unreadable right now: leave the file alone.
                Ok(Stored::Missing) | Err(_) => {}
            }
        }
        loaded.records.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(loaded)
    }

    fn path(&self, key: &str) -> PathBuf {
        self.directory.join(format!("{}.json", file_stem(key)))
    }
}

/// Keep letters, digits, `-` and `_`; escape every other byte so any key is a safe file name.
fn file_stem(key: &str) -> String {
    key.bytes()
        .map(|byte| match byte {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' => char::from(byte).to_string(),
            _ => format!("%{byte:02x}"),
        })
        .collect()
}

/// Where a runtime keeps its journal unless told otherwise: a folder of its own, named after its
/// endpoint. A runtime reads every record in its journal as belonging to a runtime that is gone,
/// so two runtimes alive at once, an old one and the one started after an update, must never
/// share a folder.
pub(crate) fn default_directory(endpoint: &str) -> io::Result<PathBuf> {
    Ok(directory_for(
        &brachio_runtime::paths::state_directory()?,
        endpoint,
    ))
}

fn directory_for(state: &Path, endpoint: &str) -> PathBuf {
    let safe: String = endpoint
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    state.join("journals").join(safe)
}

#[cfg(test)]
mod tests {
    use super::{directory_for, file_stem, Journal, SessionRecord};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn runtimes_with_different_endpoints_never_share_a_journal_folder() {
        let state = PathBuf::from("state");
        assert_ne!(
            directory_for(&state, "brachio-a-1"),
            directory_for(&state, "brachio-a-2")
        );
        assert_eq!(
            directory_for(&state, "brachio-a-1"),
            directory_for(&state, "brachio-a-1")
        );
        assert_eq!(
            directory_for(&state, "x/../y"),
            PathBuf::from("state").join("journals").join("x____y")
        );
    }

    fn record(key: &str, output_bytes: u64) -> SessionRecord {
        SessionRecord {
            key: key.to_owned(),
            cwd: PathBuf::from("work"),
            instance: "runtime-a".to_owned(),
            pid: 42,
            rows: 24,
            cols: 80,
            ansi: "prompt$ ".to_owned(),
            cursor: (0, 8),
            alternate_screen: false,
            output_bytes,
            exited: false,
            exit_code: None,
            saved_at_ms: 1_000,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("brachio-journal-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn a_saved_record_replaces_the_previous_one_and_loads_back() {
        let directory = scratch("replace");
        let journal = Journal::open(&directory).expect("open journal");
        journal.save(&record("task/one", 5)).expect("save first");
        journal.save(&record("task/one", 9)).expect("save second");
        journal.save(&record("task two", 1)).expect("save other");

        let loaded = journal.load().expect("load");
        let keys: Vec<_> = loaded
            .records
            .iter()
            .map(|r| (r.key.as_str(), r.output_bytes))
            .collect();
        assert_eq!(keys, [("task two", 1), ("task/one", 9)]);
        assert!(loaded.unreadable.is_empty());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn a_damaged_file_is_set_aside_without_hiding_other_records() {
        let directory = scratch("damaged");
        let journal = Journal::open(&directory).expect("open journal");
        journal.save(&record("good", 1)).expect("save");
        fs::write(directory.join("broken.json"), b"{not json").expect("write damaged file");

        let loaded = journal.load().expect("load");
        assert_eq!(loaded.records.len(), 1);
        assert_eq!(loaded.unreadable.len(), 1);
        let moved = loaded.unreadable[0]
            .file_name()
            .expect("name")
            .to_string_lossy();
        assert!(moved.starts_with("broken.json.corrupt-"), "{moved}");
        assert!(loaded.unreadable[0].exists());
        assert!(!directory.join("broken.json").exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn interrupted_scratch_write_does_not_replace_last_saved_record() {
        let directory = scratch("interrupted");
        let journal = Journal::open(&directory).expect("open journal");
        journal.save(&record("task", 7)).expect("save original");
        fs::write(directory.join(".task.json.1-0.tmp"), b"{incomplete")
            .expect("simulate interrupted write");

        let loaded = journal.load().expect("load after interruption");
        assert_eq!(loaded.records, vec![record("task", 7)]);
        assert!(loaded.unreadable.is_empty());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn damaged_record_does_not_prevent_a_subsequent_valid_save() {
        let directory = scratch("repair");
        let journal = Journal::open(&directory).expect("open journal");
        fs::write(directory.join("task.json"), b"{broken").expect("corrupt record");
        let loaded = journal.load().expect("load");
        assert_eq!(loaded.unreadable.len(), 1);
        journal.save(&record("task", 12)).expect("save replacement");
        assert_eq!(
            journal.load().expect("reload").records,
            vec![record("task", 12)]
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn removing_is_idempotent_and_keys_cannot_escape_the_directory() {
        let directory = scratch("remove");
        let journal = Journal::open(&directory).expect("open journal");
        journal.save(&record("../outside", 1)).expect("save");
        assert_eq!(file_stem("../outside"), "%2e%2e%2foutside");
        journal.remove("../outside").expect("remove");
        journal.remove("../outside").expect("remove again");
        assert!(journal.load().expect("load").records.is_empty());
        let _ = fs::remove_dir_all(directory);
    }
}
