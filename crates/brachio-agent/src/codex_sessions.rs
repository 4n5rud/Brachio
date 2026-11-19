//! Finds Codex conversations on disk.
//!
//! Codex picks its own conversation ids and writes each conversation to
//! `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl`. The first
//! line is a `session_meta` record with the id and the working directory.

use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// How many of the newest conversation files to open before giving up.
/// Keeps a restart fast for people with years of history.
const MAX_FILES_CHECKED: usize = 500;

/// The id of the newest Codex conversation started in `cwd`, if any.
#[must_use]
pub fn latest_codex_session(cwd: &Path) -> Option<String> {
    latest_session_in(&codex_home()?.join("sessions"), cwd)
}

/// Same as [`latest_codex_session`], searching `sessions` instead of the
/// default folder.
#[must_use]
pub fn latest_session_in(sessions: &Path, cwd: &Path) -> Option<String> {
    let wanted = normalize(cwd);
    let mut budget = MAX_FILES_CHECKED;
    find_newest(sessions, 0, &wanted, &mut budget)
}

#[derive(Deserialize)]
struct Record {
    #[serde(rename = "type")]
    kind: String,
    payload: Option<Meta>,
}

#[derive(Deserialize)]
struct Meta {
    id: String,
    cwd: String,
    /// Set for helper conversations Codex starts on its own.
    thread_source: Option<String>,
}

fn codex_home() -> Option<PathBuf> {
    if let Some(home) = env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(home));
    }
    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".codex"))
}

/// Walks the dated folders newest first and stops at the first conversation in `wanted`, so only
/// the newest days are listed however much history there is. Names start with the start time and
/// the folders are dated, so sorting names sorts by time. `budget` counts the files still allowed
/// to be opened.
fn find_newest(folder: &Path, depth: usize, wanted: &str, budget: &mut usize) -> Option<String> {
    let mut entries: Vec<_> = fs::read_dir(folder).ok()?.flatten().collect();
    entries.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        if *budget == 0 {
            return None;
        }
        let path = entry.path();
        let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if is_folder {
            if depth < 3 {
                if let Some(id) = find_newest(&path, depth + 1, wanted, budget) {
                    return Some(id);
                }
            }
        } else if is_rollout(&path) {
            *budget -= 1;
            if let Some(meta) = read_meta(&path) {
                if normalize(Path::new(&meta.cwd)) == wanted {
                    return Some(meta.id);
                }
            }
        }
    }
    None
}

fn is_rollout(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.starts_with("rollout-") && name.ends_with(".jsonl")
}

fn read_meta(file: &Path) -> Option<Meta> {
    let mut first_line = String::new();
    BufReader::new(File::open(file).ok()?)
        .read_line(&mut first_line)
        .ok()?;
    let record: Record = serde_json::from_str(&first_line).ok()?;
    let meta = record.payload?;
    let is_own = record.kind == "session_meta" && meta.thread_source.as_deref() != Some("subagent");
    is_own.then_some(meta)
}

/// Compares folders the way the OS does: resolved, and on Windows without case.
fn normalize(path: &Path) -> String {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = resolved.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_rollout(sessions: &Path, day: &str, name: &str, cwd: &Path, extra: &str) {
        let folder = sessions.join(day);
        fs::create_dir_all(&folder).expect("create day folder");
        let id = name
            .trim_start_matches("rollout-")
            .trim_end_matches(".jsonl");
        let line = format!(
            r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":{cwd}{extra}}}}}"#,
            cwd = serde_json::to_string(&cwd.to_string_lossy()).expect("quote cwd"),
        );
        fs::write(folder.join(name), format!("{line}\n{{}}\n")).expect("write rollout");
    }

    #[test]
    fn picks_the_newest_conversation_in_the_folder() {
        let sessions = tempfile::tempdir().expect("sessions dir");
        let task = tempfile::tempdir().expect("task dir");
        let other = tempfile::tempdir().expect("other dir");
        let root = sessions.path();
        write_rollout(
            root,
            "2026/11/17",
            "rollout-2026-11-17T09-old.jsonl",
            task.path(),
            "",
        );
        write_rollout(
            root,
            "2026/11/18",
            "rollout-2026-11-18T09-mine.jsonl",
            task.path(),
            "",
        );
        // Newer, but from another task or a helper conversation.
        write_rollout(
            root,
            "2026/11/18",
            "rollout-2026-11-18T10-other.jsonl",
            other.path(),
            "",
        );
        write_rollout(
            root,
            "2026/11/18",
            "rollout-2026-11-18T11-helper.jsonl",
            task.path(),
            r#","thread_source":"subagent""#,
        );

        let found = latest_session_in(root, task.path());

        assert_eq!(found.as_deref(), Some("2026-11-18T09-mine"));
    }

    #[test]
    fn none_without_a_match() {
        let sessions = tempfile::tempdir().expect("sessions dir");
        let task = tempfile::tempdir().expect("task dir");
        assert_eq!(latest_session_in(sessions.path(), task.path()), None);
    }
}
