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
/// How many conversations in the folder are checked for a tab's note.
const MAX_TAB_CANDIDATES: usize = 50;
/// How much of a conversation file is read to find a tab's note. The note is in the developer
/// instructions, which Codex writes before the first user message, within the first 200 KiB or so.
const MAX_TAB_NOTE_BYTES: u64 = 1024 * 1024;

/// A conversation started in the current working directory.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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

/// The start of the sentence that names a tab, see [`codex_brachio_instructions`].
const TAB_NOTE: &str = "This conversation belongs to Brachio tab ";
/// The largest `config.toml` read for the user's own developer instructions.
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// Whether `marker` can name a tab: letters, digits and inner hyphens only, so it passes through
/// `cmd.exe` and no marker is a sentence-ending prefix of another.
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

/// What Brachio adds to the developer instructions of a Codex launch: a sentence that points at
/// the skills file, when there is one, and a sentence that names the tab, so the conversation file
/// records which tab started it. One line, safe for `cmd.exe`. `None` when there is neither.
#[must_use]
pub fn codex_brachio_instructions(
    skills_file: Option<&str>,
    marker: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(path) = skills_file.filter(|path| crate::is_safe_for_cmd(path)) {
        parts.push(format!(
            "Before using Brachio, read and follow the instructions in {path}."
        ));
    }
    if let Some(marker) = marker.filter(|marker| is_tab_marker(marker)) {
        parts.push(format!("{TAB_NOTE}{marker}."));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// The `developer_instructions` a Codex launch passes: the user's own, from their Codex settings,
/// then Brachio's. A launch setting replaces the user's value, so it is kept this way. The user's
/// text is left out on Windows when it cannot pass through `cmd.exe`, which npm installs use.
#[must_use]
pub fn codex_developer_instructions(user: Option<&str>, brachio: &str) -> String {
    let separator = if cfg!(windows) { " " } else { "\n\n" };
    match user
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .filter(|text| !cfg!(windows) || crate::is_safe_for_cmd(text))
    {
        Some(user) => format!("{user}{separator}{brachio}"),
        None => brachio.to_owned(),
    }
}

/// The user's own `developer_instructions` in `config.toml` of the Codex config folder `home`, or
/// of the default one when `None`. `None` when there is none or the file cannot be read as TOML.
#[must_use]
pub fn codex_user_instructions(home: Option<&Path>) -> Option<String> {
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => codex_home()?,
    };
    let mut text = String::new();
    File::open(home.join("config.toml"))
        .ok()?
        .take(MAX_CONFIG_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    let table: toml::Table = text.parse().ok()?;
    table
        .get("developer_instructions")?
        .as_str()
        .map(str::to_owned)
}

/// The Codex conversations in one worktree, read in one pass over the newest files.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WorktreeConversations {
    /// The newest conversation started by the tab, see [`codex_worktree_conversations`].
    pub own: Option<String>,
    /// The newest conversations started in the worktree, for a person to choose from.
    pub listed: Vec<CodexConversation>,
}

/// The Codex conversations started in `cwd`, from the config folder `home` or the default one
/// when `None`, and the newest one a launch for a tab started.
///
/// `accepted` are the exact `developer_instructions` values a launch for the tab passes, see
/// [`codex_developer_instructions`]. A conversation is the tab's only when an interactive Codex
/// started it fresh, not as a copy or a helper of another conversation, and a developer message
/// before the first message of the user or the model has one of `accepted` as a whole item. Any
/// other developer item that names a tab, such as another tab's, rules the conversation out. A
/// conversation another tab or a shell started never qualifies.
#[must_use]
pub fn codex_worktree_conversations(
    home: Option<&Path>,
    cwd: &Path,
    accepted: &[String],
) -> WorktreeConversations {
    let mut found = WorktreeConversations::default();
    let Some(sessions) = sessions_folder(home) else {
        return found;
    };
    let wanted = normalize(cwd);
    let mut budget = MAX_FILES_CHECKED;
    let mut candidates = 0;
    let accepted = accepted
        .iter()
        .any(|text| text.contains(TAB_NOTE))
        .then_some(accepted);
    walk(&sessions, 0, &mut budget, &mut |path| {
        let Some((meta, mut reader)) = open_meta(path) else {
            return true;
        };
        if normalize(Path::new(&meta.cwd)) != wanted {
            return true;
        }
        candidates += 1;
        if found.own.is_none()
            && meta.is_fresh_interactive()
            && accepted.is_some_and(|accepted| started_with(&mut reader, accepted))
        {
            found.own = Some(meta.id.clone());
        }
        if found.listed.len() < MAX_CANDIDATES
            && !found.listed.iter().any(|item| item.id == meta.id)
        {
            found.listed.push(CodexConversation {
                id: meta.id,
                timestamp: meta.timestamp,
            });
        }
        let enough = found.own.is_some() && found.listed.len() >= MAX_CANDIDATES;
        !enough && candidates < MAX_TAB_CANDIDATES
    });
    found
}

/// Whether the Codex conversation `id`, from the config folder `home` or the default one, was
/// started in `cwd` by a launch that passed one of `accepted`, by the rules of
/// [`codex_worktree_conversations`]. A recorded id that has fallen out of the newest files is
/// checked this way; only file names are compared until it is found.
#[must_use]
pub fn codex_conversation_started_with(
    home: Option<&Path>,
    cwd: &Path,
    accepted: &[String],
    id: &str,
) -> bool {
    if id.is_empty()
        || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        || !accepted.iter().any(|text| text.contains(TAB_NOTE))
    {
        return false;
    }
    let Some(sessions) = sessions_folder(home) else {
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
    let Some((meta, mut reader)) = open_meta(&path) else {
        return false;
    };
    meta.id == id
        && normalize(Path::new(&meta.cwd)) == normalize(cwd)
        && meta.is_fresh_interactive()
        && started_with(&mut reader, accepted)
}

fn sessions_folder(home: Option<&Path>) -> Option<PathBuf> {
    match home {
        Some(home) => Some(home.join("sessions")),
        None => Some(codex_home()?.join("sessions")),
    }
}

/// Visits rollout files newest first, opening at most `budget` of them, until `visit` returns
/// `false`.
fn walk(
    folder: &Path,
    depth: usize,
    budget: &mut usize,
    visit: &mut dyn FnMut(&Path) -> bool,
) -> bool {
    for (path, is_folder) in newest_entries(folder, depth, *budget) {
        if *budget == 0 {
            return false;
        }
        if is_folder {
            if !walk(&path, depth + 1, budget, visit) {
                return false;
            }
        } else {
            *budget -= 1;
            if !visit(&path) {
                return false;
            }
        }
    }
    true
}

/// The conversation's metadata, with the file left open just after it, so the rest is read
/// without reading the first line again.
fn open_meta(path: &Path) -> Option<(Meta, BufReader<std::io::Take<File>>)> {
    let mut reader = BufReader::new(File::open(path).ok()?.take(MAX_TAB_NOTE_BYTES));
    let mut first_line = String::new();
    reader
        .by_ref()
        .take(MAX_HEADER_BYTES)
        .read_line(&mut first_line)
        .ok()?;
    let meta = meta_from(&first_line)?;
    Some((meta, reader))
}

/// Whether a developer message before the first other message has one of `accepted` as a whole
/// item, and no developer item there names a tab otherwise. `reader` is just after the metadata.
fn started_with(reader: &mut impl BufRead, accepted: &[String]) -> bool {
    let mut line = String::new();
    let mut matched = false;
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
        let Ok(record) = serde_json::from_str::<serde_json::Value>(&line) else {
            return false;
        };
        if record["type"] != "response_item" || record["payload"]["type"] != "message" {
            continue;
        }
        if record["payload"]["role"] != "developer" {
            return matched;
        }
        let texts = record["payload"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item["text"].as_str());
        for text in texts {
            if accepted.iter().any(|exact| exact == text) {
                matched = true;
            } else if text.contains(TAB_NOTE) {
                return false;
            }
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
    meta_from(&first_line)
}

fn meta_from(first_line: &str) -> Option<Meta> {
    let record: Record = serde_json::from_str(first_line).ok()?;
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
    /// `(role, items)`.
    fn write_conversation(
        sessions: &Path,
        name: &str,
        cwd: &Path,
        meta: serde_json::Value,
        messages: &[(&str, &[&str])],
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
        for (role, items) in messages {
            let content: Vec<_> = items
                .iter()
                .map(|body| serde_json::json!({"type": "input_text", "text": body}))
                .collect();
            let record = serde_json::json!({
                "type": "response_item",
                "payload": {"type": "message", "role": role, "content": content},
            });
            text.push_str(&record.to_string());
            text.push('\n');
        }
        fs::write(folder.join(format!("{name}.jsonl")), text).expect("write conversation");
    }

    /// What a launch for the tab `marker` passes, with and without the user's own instructions.
    fn accepted(marker: &str, user: Option<&str>) -> Vec<String> {
        let brachio =
            codex_brachio_instructions(Some("/fake-home/x.md"), Some(marker)).expect("text");
        let mut accepted = vec![codex_developer_instructions(None, &brachio)];
        if user.is_some() {
            accepted.push(codex_developer_instructions(user, &brachio));
        }
        accepted
    }

    fn passed(marker: &str) -> String {
        accepted(marker, None).remove(0)
    }

    const TAB_A: &str = "0123456789abcdef0123456789abcdef-agent";
    const TAB_B: &str =
        "0123456789abcdef0123456789abcdef-agent-22222222-2222-4222-8222-222222222222";
    const NONE: &[&str] = &[];

    #[test]
    fn a_tab_finds_only_the_conversation_its_own_launch_started() {
        let home = tempfile::tempdir().expect("codex home");
        let task = tempfile::tempdir().expect("worktree");
        let other = tempfile::tempdir().expect("other worktree");
        let sessions = home.path().join("sessions");
        let none = serde_json::json!({});
        let (a, b) = (passed(TAB_A), passed(TAB_B));
        let skill = format!("Skill notes. {TAB_NOTE}{TAB_A}.");
        let write =
            |name: &str, cwd: &Path, meta: &serde_json::Value, messages: &[(&str, &[&str])]| {
                write_conversation(&sessions, name, cwd, meta.clone(), messages);
            };
        write(
            "rollout-01-tab-a",
            task.path(),
            &none,
            &[
                ("developer", &["base"]),
                ("developer", &[&a, "skills"]),
                ("user", &["fix it"]),
            ],
        );
        write(
            "rollout-02-tab-b",
            task.path(),
            &none,
            &[("developer", &[&b]), ("user", &["fix it"])],
        );
        // Newer than both, in the same worktree, but not started by tab A.
        write(
            "rollout-03-shell",
            task.path(),
            &none,
            &[("developer", &["base"]), ("user", &["hi"])],
        );
        write("rollout-04-typed", task.path(), &none, &[("user", &[&a])]);
        write(
            "rollout-05-late",
            task.path(),
            &none,
            &[
                ("developer", &["base"]),
                ("user", &["hi"]),
                ("developer", &[&a]),
            ],
        );
        write(
            "rollout-06-fork",
            task.path(),
            &serde_json::json!({"forked_from_id": "01-tab-a"}),
            &[("developer", &[&a]), ("user", &["x"])],
        );
        write(
            "rollout-07-helper",
            task.path(),
            &serde_json::json!({"thread_source": "subagent", "parent_thread_id": "01-tab-a"}),
            &[("developer", &[&a])],
        );
        write(
            "rollout-08-exec",
            task.path(),
            &serde_json::json!({"source": "exec"}),
            &[("developer", &[&a])],
        );
        write(
            "rollout-09-elsewhere",
            other.path(),
            &none,
            &[("developer", &[&a])],
        );
        write(
            "rollout-10-in-a-skill",
            task.path(),
            &none,
            &[("developer", &[&skill]), ("user", &["x"])],
        );
        write(
            "rollout-11-two-tabs",
            task.path(),
            &none,
            &[("developer", &[&a, &b]), ("user", &["x"])],
        );
        write(
            "rollout-12-prefix",
            task.path(),
            &none,
            &[("developer", &[&format!("{a} extra")]), ("user", &["x"])],
        );

        let find = |marker| {
            codex_worktree_conversations(Some(home.path()), task.path(), &accepted(marker, None))
        };
        assert_eq!(find(TAB_A).own.as_deref(), Some("01-tab-a"));
        assert_eq!(find(TAB_B).own.as_deref(), Some("02-tab-b"));
        assert_eq!(find("0123456789abcdef0123456789abcdef-agent-3").own, None);
        let listed: Vec<_> = find(TAB_A).listed.into_iter().map(|item| item.id).collect();
        assert_eq!(listed.len(), MAX_CANDIDATES);
        assert!(!listed
            .iter()
            .any(|id| id == "07-helper" || id == "09-elsewhere"));
        assert_eq!(
            codex_worktree_conversations(Some(home.path()), other.path(), &accepted(TAB_B, None))
                .own,
            None
        );
        assert_eq!(
            codex_worktree_conversations(Some(home.path()), task.path(), &["base".to_owned()]).own,
            None,
            "instructions that name no tab never pick a conversation"
        );
    }

    #[test]
    fn the_newest_conversation_of_the_tab_wins() {
        let home = tempfile::tempdir().expect("codex home");
        let task = tempfile::tempdir().expect("worktree");
        let sessions = home.path().join("sessions");
        let a = passed(TAB_A);
        for name in ["rollout-01-first", "rollout-02-after-new"] {
            write_conversation(
                &sessions,
                name,
                task.path(),
                serde_json::json!({}),
                &[("developer", &[&a]), ("user", &["go"])],
            );
        }
        let found =
            codex_worktree_conversations(Some(home.path()), task.path(), &accepted(TAB_A, None));
        assert_eq!(found.own.as_deref(), Some("02-after-new"));
    }

    #[test]
    fn the_users_own_instructions_are_kept_and_still_name_the_tab() {
        let home = tempfile::tempdir().expect("codex home");
        let task = tempfile::tempdir().expect("worktree");
        fs::write(
            home.path().join("config.toml"),
            "model = \"x\"\ndeveloper_instructions = \"Answer in Korean.\"\n",
        )
        .expect("config");
        let user = codex_user_instructions(Some(home.path()));
        assert_eq!(user.as_deref(), Some("Answer in Korean."));
        let brachio =
            codex_brachio_instructions(Some("/fake-home/x.md"), Some(TAB_A)).expect("text");
        let both = codex_developer_instructions(user.as_deref(), &brachio);
        assert!(both.starts_with("Answer in Korean."));
        assert!(both.ends_with(&brachio));
        write_conversation(
            &home.path().join("sessions"),
            "rollout-01-mine",
            task.path(),
            serde_json::json!({}),
            &[("developer", &[&both]), ("user", &["go"])],
        );
        let find = |accepted: &[String]| {
            codex_worktree_conversations(Some(home.path()), task.path(), accepted).own
        };
        assert_eq!(
            find(&accepted(TAB_A, user.as_deref())).as_deref(),
            Some("01-mine")
        );
        assert_eq!(find(&accepted(TAB_A, Some("Something else."))), None);
        assert_eq!(find(&accepted(TAB_B, user.as_deref())), None);

        for broken in [
            "developer_instructions = 3\n",
            "developer_instructions = \"x",
            "",
        ] {
            fs::write(home.path().join("config.toml"), broken).expect("config");
            assert_eq!(
                codex_user_instructions(Some(home.path())),
                None,
                "{broken:?}"
            );
        }
        assert_eq!(codex_developer_instructions(Some("  "), &brachio), brachio);
        assert_eq!(codex_brachio_instructions(None, Some("bad marker")), None);
        assert_eq!(
            codex_brachio_instructions(None, Some(TAB_A)).as_deref(),
            Some(
                "This conversation belongs to Brachio tab 0123456789abcdef0123456789abcdef-agent."
            )
        );
    }

    #[test]
    fn a_conversation_is_looked_for_only_in_the_tabs_own_account() {
        let first = tempfile::tempdir().expect("first account");
        let second = tempfile::tempdir().expect("second account");
        let task = tempfile::tempdir().expect("worktree");
        let a = passed(TAB_A);
        write_conversation(
            &second.path().join("sessions"),
            "rollout-01-other-account",
            task.path(),
            serde_json::json!({}),
            &[("developer", &[&a]), ("user", &["go"])],
        );
        let accepted = accepted(TAB_A, None);
        assert_eq!(
            codex_worktree_conversations(Some(first.path()), task.path(), &accepted).own,
            None
        );
        assert!(!codex_conversation_started_with(
            Some(first.path()),
            task.path(),
            &accepted,
            "01-other-account"
        ));
        assert_eq!(
            codex_worktree_conversations(Some(second.path()), task.path(), &accepted)
                .own
                .as_deref(),
            Some("01-other-account")
        );
    }

    #[test]
    fn a_recorded_id_is_resumed_only_while_its_file_still_names_the_tab() {
        let home = tempfile::tempdir().expect("codex home");
        let task = tempfile::tempdir().expect("worktree");
        let sessions = home.path().join("sessions");
        let a = passed(TAB_A);
        write_conversation(
            &sessions,
            "rollout-2026-11-27T01-00-00-01a2",
            task.path(),
            serde_json::json!({}),
            &[("developer", &[&a]), ("user", &["go"])],
        );
        write_conversation(
            &sessions,
            "rollout-2026-11-27T02-00-00-02b3",
            task.path(),
            serde_json::json!({}),
            &[("developer", &["base"]), ("user", &["go"])],
        );
        let started = |marker, id| {
            codex_conversation_started_with(
                Some(home.path()),
                task.path(),
                &accepted(marker, None),
                id,
            )
        };
        assert!(started(TAB_A, "2026-11-27T01-00-00-01a2"));
        assert!(!started(TAB_A, "2026-11-27T02-00-00-02b3"), "no tab named");
        assert!(
            !started(TAB_A, "01a2"),
            "the id must be the conversation's own"
        );
        assert!(!started(TAB_A, "missing"));
        assert!(!started(TAB_A, "../x"));
        assert!(!started(TAB_B, "2026-11-27T01-00-00-01a2"));
    }

    #[test]
    fn the_search_stops_after_the_newest_candidates_of_the_worktree() {
        let home = tempfile::tempdir().expect("codex home");
        let task = tempfile::tempdir().expect("worktree");
        let sessions = home.path().join("sessions");
        let a = passed(TAB_A);
        write_conversation(
            &sessions,
            "rollout-000-mine",
            task.path(),
            serde_json::json!({}),
            &[("developer", &[&a]), ("user", &["go"])],
        );
        for number in 1..=MAX_TAB_CANDIDATES {
            write_conversation(
                &sessions,
                &format!("rollout-{number:03}-shell"),
                task.path(),
                serde_json::json!({}),
                &[("developer", NONE), ("user", &["hi"])],
            );
        }
        let found =
            codex_worktree_conversations(Some(home.path()), task.path(), &accepted(TAB_A, None));
        assert_eq!(found.own, None, "older than the newest candidates");
        assert!(codex_conversation_started_with(
            Some(home.path()),
            task.path(),
            &accepted(TAB_A, None),
            "000-mine"
        ));
    }

    #[test]
    fn a_tab_marker_is_plain_and_never_a_prefix_inside_a_note() {
        assert!(is_tab_marker(TAB_A) && is_tab_marker(TAB_B));
        for bad in ["", "-a", "a-", "a b", "a:agent", "a.b", "a%b"] {
            assert!(!is_tab_marker(bad), "{bad:?}");
        }
        assert!(!passed(TAB_B).contains(&format!("{TAB_NOTE}{TAB_A}.")));
    }

    #[test]
    fn none_without_a_match() {
        let sessions = tempfile::tempdir().expect("sessions dir");
        let task = tempfile::tempdir().expect("task dir");
        assert_eq!(latest_session_in(sessions.path(), task.path()), None);
    }
}
