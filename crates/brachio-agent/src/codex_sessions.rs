//! Finds Codex conversations on disk.
//!
//! Codex picks its own conversation ids and writes each conversation to
//! `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl`. The first
//! line is a `session_meta` record with the id and the working directory.

use std::cell::Cell;
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
/// How many conversations in the folder are checked for a tab's note.
const MAX_TAB_CANDIDATES: usize = 50;
/// How much of a conversation file is read to find a tab's note. The note is in the developer
/// instructions, which Codex writes before the first user message, within the first 200 KiB or so.
const MAX_TAB_NOTE_BYTES: u64 = 1024 * 1024;

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
    codex_home().map_or_else(Vec::new, |home| codex_conversations_in(&home, cwd))
}

/// Finds conversations for a selected Codex configuration folder.
#[must_use]
pub fn codex_conversations_in(home: &Path, cwd: &Path) -> Vec<CodexConversation> {
    conversations_in(&home.join("sessions"), cwd, MAX_CANDIDATES)
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
    /// What started the conversation: `"cli"` for the interactive Codex a tab runs.
    #[serde(default)]
    source: Option<serde_json::Value>,
    /// Set when the conversation was copied from another one.
    #[serde(default)]
    forked_from_id: Option<serde_json::Value>,
    /// Set when another conversation started this one.
    #[serde(default)]
    parent_thread_id: Option<serde_json::Value>,
}

impl Meta {
    /// Started fresh by an interactive Codex: not copied from or started by another conversation.
    fn is_fresh_interactive(&self) -> bool {
        self.source.as_ref().and_then(serde_json::Value::as_str) == Some("cli")
            && self.forked_from_id.is_none()
            && self.parent_thread_id.is_none()
    }
}

/// The sentence Brachio adds to the developer instructions of a Codex launch, naming the tab that
/// launched it, so the conversation file records which tab it belongs to.
#[must_use]
pub fn codex_tab_note(marker: &str) -> String {
    format!("This conversation belongs to Brachio tab {marker}.")
}

/// Whether `marker` can name a tab in a note: letters, digits and inner hyphens only, so it passes
/// through `cmd.exe` and no marker is a note-ending prefix of another.
#[must_use]
pub fn is_tab_marker(marker: &str) -> bool {
    !marker.is_empty()
        && marker.len() <= 128
        && !marker.starts_with('-')
        && !marker.ends_with('-')
        && marker
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The newest Codex conversation started in `cwd` by a launch for the tab `marker`, from the
/// config folder `home`, or the default one when `None`.
///
/// Only a conversation an interactive Codex started fresh counts: not a copy or a helper of
/// another conversation, and only when the tab's note is in a developer message before the first
/// message of the user or the model. A conversation another tab or a shell started has no such
/// note, so it is never returned. `None` when no conversation can be confirmed.
#[must_use]
pub fn codex_conversation_for_tab(home: Option<&Path>, cwd: &Path, marker: &str) -> Option<String> {
    if !is_tab_marker(marker) {
        return None;
    }
    let sessions = match home {
        Some(home) => home.join("sessions"),
        None => codex_home()?.join("sessions"),
    };
    let note = codex_tab_note(marker);
    let wanted = normalize(cwd);
    let mut budget = MAX_FILES_CHECKED;
    let candidates = Cell::new(MAX_TAB_CANDIDATES);
    let path = find_rollout(&sessions, 0, &mut budget, &|path| {
        found_in(path, &wanted, &note, &candidates)
    })?;
    read_meta(&path).map(|meta| meta.id)
}

/// Whether the Codex conversation `id`, from the config folder `home` or the default one, was
/// started in `cwd` by a launch for the tab `marker`, by the same rules as
/// [`codex_conversation_for_tab`]. A recorded id is checked again before it is resumed.
#[must_use]
pub fn codex_conversation_started_by_tab(
    home: Option<&Path>,
    cwd: &Path,
    marker: &str,
    id: &str,
) -> bool {
    if !is_tab_marker(marker)
        || id.is_empty()
        || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return false;
    }
    let Some(sessions) = home
        .map(|home| home.join("sessions"))
        .or_else(|| Some(codex_home()?.join("sessions")))
    else {
        return false;
    };
    let suffix = format!("-{id}.jsonl");
    let mut budget = MAX_FILES_CHECKED;
    let Some(path) = find_rollout(&sessions, 0, &mut budget, &|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(&suffix))
    }) else {
        return false;
    };
    let one_candidate = Cell::new(1);
    read_meta(&path).is_some_and(|meta| meta.id == id)
        && found_in(
            &path,
            &normalize(cwd),
            &codex_tab_note(marker),
            &one_candidate,
        )
}

/// Whether the file at `path` is a conversation in `wanted` that the tab with `note` started.
/// `candidates` counts the conversations in `wanted` still allowed to be read.
fn found_in(path: &Path, wanted: &str, note: &str, candidates: &Cell<usize>) -> bool {
    let Some(meta) = read_meta(path) else {
        return false;
    };
    if candidates.get() == 0 || normalize(Path::new(&meta.cwd)) != wanted {
        return false;
    }
    candidates.set(candidates.get() - 1);
    meta.is_fresh_interactive() && carries_note(path, note)
}

/// Whether a developer message before the first other message contains `note`.
fn carries_note(path: &Path, note: &str) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut reader = BufReader::new(file.take(MAX_TAB_NOTE_BYTES));
    let mut line = String::new();
    let mut first = true;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return false,
            Ok(_) => {}
        }
        // A line cut off by the read limit is not trusted.
        if !line.ends_with('\n') {
            return false;
        }
        if std::mem::take(&mut first) {
            continue;
        }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(&line) else {
            return false;
        };
        if record["type"] != "response_item" || record["payload"]["type"] != "message" {
            continue;
        }
        match record["payload"]["role"].as_str() {
            Some("developer") => {
                let noted = record["payload"]["content"]
                    .as_array()
                    .is_some_and(|items| {
                        items
                            .iter()
                            .filter_map(|item| item["text"].as_str())
                            .any(|text| text.contains(note))
                    });
                if noted {
                    return true;
                }
            }
            _ => return false,
        }
    }
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
    fn a_selected_account_only_lists_its_own_conversations() {
        let first = tempfile::tempdir().expect("first account");
        let second = tempfile::tempdir().expect("second account");
        let task = tempfile::tempdir().expect("task");
        write_rollout(
            &first.path().join("sessions"),
            "2026/11/17",
            "rollout-first.jsonl",
            task.path(),
            "",
        );
        write_rollout(
            &second.path().join("sessions"),
            "2026/11/17",
            "rollout-second.jsonl",
            task.path(),
            "",
        );
        let listed = codex_conversations_in(first.path(), task.path());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "first");
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

    /// Writes a conversation as Codex does: its metadata, then the given messages as
    /// `(role, text)`.
    fn write_conversation(
        sessions: &Path,
        name: &str,
        cwd: &Path,
        meta: serde_json::Value,
        messages: &[(&str, &str)],
    ) {
        let folder = sessions.join("2026/11/27");
        fs::create_dir_all(&folder).expect("day folder");
        let id = name.trim_start_matches("rollout-");
        let mut payload = serde_json::json!({
            "id": id,
            "cwd": cwd.to_string_lossy(),
            "source": "cli",
            "originator": "codex-tui",
        });
        for (key, value) in meta.as_object().expect("object") {
            payload[key] = value.clone();
        }
        let mut text = serde_json::json!({"type": "session_meta", "payload": payload}).to_string();
        text.push('\n');
        text.push_str(r#"{"type":"event_msg","payload":{"type":"task_started"}}"#);
        text.push('\n');
        for (role, body) in messages {
            let record = serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": role,
                    "content": [{"type": "input_text", "text": body}],
                },
            });
            text.push_str(&record.to_string());
            text.push('\n');
        }
        fs::write(folder.join(format!("{name}.jsonl")), text).expect("write conversation");
    }

    fn instructions(marker: &str) -> String {
        format!(
            "Before using Brachio, read and follow the instructions in /fake-home/x.md. {}",
            codex_tab_note(marker)
        )
    }

    const TAB_A: &str = "0123456789abcdef0123456789abcdef-agent";
    const TAB_B: &str =
        "0123456789abcdef0123456789abcdef-agent-22222222-2222-4222-8222-222222222222";

    #[test]
    fn a_tab_finds_only_the_conversation_its_own_launch_started() {
        let home = tempfile::tempdir().expect("codex home");
        let task = tempfile::tempdir().expect("worktree");
        let other = tempfile::tempdir().expect("other worktree");
        let sessions = home.path().join("sessions");
        let none = serde_json::json!({});
        let (a, b) = (instructions(TAB_A), instructions(TAB_B));
        write_conversation(
            &sessions,
            "rollout-01-tab-a",
            task.path(),
            none.clone(),
            &[("developer", "base"), ("developer", &a), ("user", "fix it")],
        );
        write_conversation(
            &sessions,
            "rollout-02-tab-b",
            task.path(),
            none.clone(),
            &[("developer", &b), ("user", "fix it")],
        );
        // Newer than both, in the same worktree, but started some other way.
        write_conversation(
            &sessions,
            "rollout-03-shell",
            task.path(),
            none.clone(),
            &[("developer", "base"), ("user", "hello")],
        );
        write_conversation(
            &sessions,
            "rollout-04-typed",
            task.path(),
            none.clone(),
            &[("user", &a)],
        );
        write_conversation(
            &sessions,
            "rollout-05-late",
            task.path(),
            none.clone(),
            &[("developer", "base"), ("user", "hi"), ("developer", &a)],
        );
        write_conversation(
            &sessions,
            "rollout-06-fork",
            task.path(),
            serde_json::json!({"forked_from_id": "01-tab-a"}),
            &[("developer", &a), ("user", "x")],
        );
        write_conversation(
            &sessions,
            "rollout-07-helper",
            task.path(),
            serde_json::json!({"thread_source": "subagent", "parent_thread_id": "01-tab-a"}),
            &[("developer", &a)],
        );
        write_conversation(
            &sessions,
            "rollout-08-exec",
            task.path(),
            serde_json::json!({"source": "exec"}),
            &[("developer", &a)],
        );
        write_conversation(
            &sessions,
            "rollout-09-elsewhere",
            other.path(),
            none,
            &[("developer", &a)],
        );

        let find = |marker| codex_conversation_for_tab(Some(home.path()), task.path(), marker);
        assert_eq!(find(TAB_A).as_deref(), Some("01-tab-a"));
        assert_eq!(find(TAB_B).as_deref(), Some("02-tab-b"));
        assert_eq!(find("0123456789abcdef0123456789abcdef-agent-3"), None);
        assert_eq!(find("bad marker"), None);
        assert_eq!(find(""), None);
        assert_eq!(
            codex_conversation_for_tab(Some(home.path()), other.path(), TAB_B),
            None
        );
    }

    #[test]
    fn a_recorded_id_is_resumed_only_while_its_file_still_names_the_tab() {
        let home = tempfile::tempdir().expect("codex home");
        let task = tempfile::tempdir().expect("worktree");
        let sessions = home.path().join("sessions");
        let a = instructions(TAB_A);
        write_conversation(
            &sessions,
            "rollout-2026-11-27T01-00-00-01a2",
            task.path(),
            serde_json::json!({}),
            &[("developer", &a), ("user", "go")],
        );
        write_conversation(
            &sessions,
            "rollout-2026-11-27T02-00-00-02b3",
            task.path(),
            serde_json::json!({}),
            &[("developer", "base"), ("user", "go")],
        );
        let started =
            |id| codex_conversation_started_by_tab(Some(home.path()), task.path(), TAB_A, id);
        assert!(started("2026-11-27T01-00-00-01a2"));
        assert!(!started("2026-11-27T02-00-00-02b3"), "no note");
        assert!(!started("01a2"), "the id must be the conversation's own");
        assert!(!started("missing"));
        assert!(!started("../x"));
        assert!(!codex_conversation_started_by_tab(
            Some(home.path()),
            task.path(),
            TAB_B,
            "2026-11-27T01-00-00-01a2"
        ));
    }

    #[test]
    fn a_tab_marker_is_plain_and_never_a_prefix_inside_a_note() {
        assert!(is_tab_marker(TAB_A) && is_tab_marker(TAB_B));
        for bad in ["", "-a", "a-", "a b", "a:agent", "a.b", "a%b"] {
            assert!(!is_tab_marker(bad), "{bad:?}");
        }
        assert!(!codex_tab_note(TAB_B).contains(&codex_tab_note(TAB_A)));
    }

    #[test]
    fn none_without_a_match() {
        let sessions = tempfile::tempdir().expect("sessions dir");
        let task = tempfile::tempdir().expect("task dir");
        assert_eq!(latest_session_in(sessions.path(), task.path()), None);
    }
}
