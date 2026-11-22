//! Finds Codex conversations on disk.
//!
//! Codex picks its own conversation ids and writes each conversation to
//! `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl`. The first
//! line is a `session_meta` record with the id and the working directory.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many of the newest conversation files to open before giving up.
/// Keeps a restart fast for people with years of history.
const MAX_FILES_CHECKED: usize = 500;
const MAX_HEADER_BYTES: u64 = 128 * 1024;
const MAX_CANDIDATES: usize = 10;

/// A conversation started in the current working directory.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CodexConversation {
    /// The CLI conversation ID, separate from the Brachio tab ID.
    pub id: String,
    /// The time reported in the session metadata, when available.
    pub timestamp: Option<String>,
}

/// Finds the most recent Codex conversations in `cwd` without selecting one.
#[must_use]
pub fn codex_conversations(cwd: &Path) -> Vec<CodexConversation> {
    codex_home().map_or_else(Vec::new, |home| {
        conversations_in(&home.join("sessions"), cwd, MAX_CANDIDATES)
    })
}

/// The id of the newest Codex conversation started in `cwd`, if any.
#[must_use]
pub fn latest_codex_session(cwd: &Path) -> Option<String> {
    latest_session_in(&codex_home()?.join("sessions"), cwd)
}

/// Same as [`latest_codex_session`], searching `sessions` instead of the
/// default folder.
#[must_use]
pub fn latest_session_in(sessions: &Path, cwd: &Path) -> Option<String> {
    conversations_in(sessions, cwd, 1)
        .into_iter()
        .next()
        .map(|session| session.id)
}

/// The rollout file of a Codex conversation: the one named for `session_id` when it is known and
/// present, otherwise the newest conversation started in `cwd`.
#[must_use]
pub fn codex_rollout_path(cwd: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    rollout_in(&codex_home()?.join("sessions"), cwd, session_id)
}

/// Same as [`codex_rollout_path`], searching `sessions` instead of the default folder.
#[must_use]
pub fn rollout_in(sessions: &Path, cwd: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    if let Some(id) =
        session_id.filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        let suffix = format!("-{id}.jsonl");
        let mut budget = MAX_FILES_CHECKED;
        let named = find_rollout(sessions, 0, &mut budget, &|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(&suffix))
        });
        if named.is_some() {
            return named;
        }
    }
    let wanted = normalize(cwd);
    let mut budget = MAX_FILES_CHECKED;
    find_rollout(sessions, 0, &mut budget, &|path| {
        read_meta(path).is_some_and(|meta| normalize(Path::new(&meta.cwd)) == wanted)
    })
}

fn find_rollout(
    folder: &Path,
    depth: usize,
    budget: &mut usize,
    matches: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    for (path, is_folder) in newest_entries(folder, depth, *budget) {
        if *budget == 0 {
            break;
        }
        if is_folder {
            if let Some(found) = find_rollout(&path, depth + 1, budget, matches) {
                return Some(found);
            }
        } else {
            *budget -= 1;
            if matches(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn conversations_in(sessions: &Path, cwd: &Path, limit: usize) -> Vec<CodexConversation> {
    let wanted = normalize(cwd);
    let mut budget = MAX_FILES_CHECKED;
    let mut found = Vec::new();
    collect(sessions, 0, &wanted, &mut budget, limit, &mut found);
    found
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
    timestamp: Option<String>,
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
fn collect(
    folder: &Path,
    depth: usize,
    wanted: &str,
    budget: &mut usize,
    limit: usize,
    found: &mut Vec<CodexConversation>,
) {
    for (path, is_folder) in newest_entries(folder, depth, *budget) {
        if *budget == 0 || found.len() >= limit {
            break;
        }
        if is_folder {
            collect(&path, depth + 1, wanted, budget, limit, found);
        } else {
            *budget -= 1;
            if let Some(meta) = read_meta(&path) {
                if normalize(Path::new(&meta.cwd)) == wanted
                    && !found.iter().any(|item| item.id == meta.id)
                {
                    found.push(CodexConversation {
                        id: meta.id,
                        timestamp: meta.timestamp,
                    });
                }
            }
        }
    }
}

/// The `cap` newest rollout files and subfolders of `folder`, newest first, each with whether it is
/// a folder. The listing is streamed and only `cap` names are ever held, so a huge folder costs
/// time but not memory. Anything else in the folder is skipped without using a slot. At most
/// `cap` files can be opened anyway; only more than `cap` newer subfolders hide an older one.
fn newest_entries(folder: &Path, depth: usize, cap: usize) -> Vec<(PathBuf, bool)> {
    let Ok(directory) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut newest: BinaryHeap<Reverse<(OsString, bool)>> = BinaryHeap::with_capacity(cap + 1);
    for entry in directory.flatten() {
        let name = entry.file_name();
        let rollout = is_rollout(Path::new(&name));
        let is_folder = !rollout && depth < 3 && entry.file_type().is_ok_and(|kind| kind.is_dir());
        if rollout || is_folder {
            newest.push(Reverse((name, is_folder)));
            if newest.len() > cap {
                newest.pop();
            }
        }
    }
    newest
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse((name, is_folder))| (folder.join(name), is_folder))
        .collect()
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
    BufReader::new(File::open(file).ok()?.take(MAX_HEADER_BYTES))
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
            r#","timestamp":"2026-11-18T09:00:00Z""#,
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
        assert_eq!(
            conversations_in(root, task.path(), 10),
            [
                CodexConversation {
                    id: "2026-11-18T09-mine".into(),
                    timestamp: Some("2026-11-18T09:00:00Z".into()),
                },
                CodexConversation {
                    id: "2026-11-17T09-old".into(),
                    timestamp: None
                },
            ]
        );
    }

    #[test]
    fn a_huge_folder_keeps_only_the_newest_names_and_skips_unrelated_entries() {
        let folder = tempfile::tempdir().expect("folder");
        for minute in 0..30 {
            fs::write(
                folder.path().join(format!("rollout-t{minute:02}.jsonl")),
                "",
            )
            .expect("file");
        }
        fs::write(folder.path().join("zzz-notes.txt"), "").expect("unrelated");

        let names: Vec<_> = newest_entries(folder.path(), 3, 5)
            .into_iter()
            .map(|(path, is_folder)| (path.file_name().expect("name").to_owned(), is_folder))
            .collect();

        let expected: Vec<_> = (25..30)
            .rev()
            .map(|minute| (format!("rollout-t{minute:02}.jsonl").into(), false))
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn a_crowded_day_is_scanned_only_up_to_the_budget_newest_first() {
        let sessions = tempfile::tempdir().expect("sessions dir");
        let task = tempfile::tempdir().expect("task dir");
        let other = tempfile::tempdir().expect("other dir");
        let root = sessions.path();
        write_rollout(
            root,
            "2026/11/17",
            "rollout-2026-11-17T01-mine.jsonl",
            task.path(),
            "",
        );
        for number in 0..MAX_FILES_CHECKED + 100 {
            write_rollout(
                root,
                "2026/11/18",
                &format!("rollout-2026-11-18T{number:04}-x.jsonl"),
                other.path(),
                "",
            );
        }

        assert_eq!(
            latest_session_in(root, task.path()),
            None,
            "past the scan budget"
        );
        write_rollout(
            root,
            "2026/11/18",
            "rollout-2026-11-18T9999-new.jsonl",
            task.path(),
            "",
        );
        assert_eq!(
            latest_session_in(root, task.path()).as_deref(),
            Some("2026-11-18T9999-new")
        );
    }

    #[test]
    fn none_without_a_match() {
        let sessions = tempfile::tempdir().expect("sessions dir");
        let task = tempfile::tempdir().expect("task dir");
        assert_eq!(latest_session_in(sessions.path(), task.path()), None);
    }
}
