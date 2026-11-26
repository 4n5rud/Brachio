//! Reads every local session log of an agent provider, not only Brachio's own tasks, and turns it
//! into per-day token buckets. Reading is incremental: each log keeps a [`FileState`] with the byte
//! offset it stopped at, so a rescan reads only what was appended. Nothing leaves the machine.
//!
//! A provider is a [`UsageProvider`]: an id, a name, an icon and the readers for its logs. Adding
//! a provider means implementing the trait and listing it in [`providers`].

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::session_tokens::{TokenUsage, MAX_LINE_BYTES, MAX_READ_BYTES};

/// Most log files listed for one provider; the newest are kept.
pub const MAX_LOG_FILES: usize = 5000;

/// One log of a provider and the session it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFile {
    /// Where the log is.
    pub path: PathBuf,
    /// The conversation id the log records.
    pub session: String,
}

/// Tokens one session used on one day, in one project folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    /// UTC date, `YYYY-MM-DD`.
    pub day: String,
    /// The working directory the session ran in; empty when the log does not say.
    pub project: String,
    /// The conversation id.
    pub session: String,
    /// What was added since the last scan.
    pub usage: TokenUsage,
}

/// Where a scan of one log stopped, saved between scans.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileState {
    /// Bytes already read.
    pub offset: u64,
    /// The working directory seen so far.
    pub project: String,
    /// Claude: the message id of the last counted line and its usage, to count a repeat once.
    pub last_id: Option<String>,
    /// Usage of the last counted message.
    pub last_usage: TokenUsage,
    /// Codex: the cumulative total already counted.
    pub previous_total: TokenUsage,
    /// The offset is inside a line longer than [`MAX_READ_BYTES`]; the rest of it up to its
    /// newline is skipped. States saved before this field existed load as `false`.
    #[serde(default)]
    pub skipping: bool,
}

/// A source of session logs. The app lists providers; the rest of the code does not name them.
pub trait UsageProvider: Send + Sync {
    /// Stable id, such as `claude_code`; stored with the totals.
    fn id(&self) -> &'static str;
    /// Name shown in the interface.
    fn display_name(&self) -> &'static str;
    /// Icon key the interface maps to a brand mark.
    fn icon(&self) -> &'static str;
    /// The provider's logs, newest first, at most [`MAX_LOG_FILES`].
    fn log_files(&self) -> Vec<LogFile>;
    /// Reads what was appended to `file` since `state` and returns it as buckets.
    ///
    /// # Errors
    /// Fails when the log cannot be opened or read.
    fn scan(&self, file: &LogFile, state: &mut FileState) -> io::Result<Scanned>;
}

/// The result of reading one log.
#[derive(Debug, Default)]
pub struct Scanned {
    /// New usage, one bucket per day.
    pub buckets: Vec<Bucket>,
    /// Bytes the saved offset moved forward this time, including skipped ones. Zero means
    /// nothing changed and the state need not be saved.
    pub bytes: u64,
}

/// Claude Code: `~/.claude/projects/<folder>/<session>.jsonl`.
pub struct ClaudeProvider;
/// Codex: `~/.codex/sessions/YYYY/MM/DD/rollout-<time>-<session>.jsonl`.
pub struct CodexProvider;

/// Grok: `~/.grok/sessions/**/<session>/updates.jsonl`. The layout and field names follow
/// third-party descriptions and have not been checked against an installed Grok.
pub struct GrokProvider;

/// Every provider the app knows.
#[must_use]
pub fn providers() -> Vec<Box<dyn UsageProvider>> {
    vec![
        Box::new(ClaudeProvider),
        Box::new(CodexProvider),
        Box::new(GrokProvider),
    ]
}

fn home_dir(var: &str, default: &str) -> Option<PathBuf> {
    if let Some(dir) = env::var_os(var) {
        return Some(PathBuf::from(dir));
    }
    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(default))
}

fn count(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn day_of(record: &Value) -> Option<String> {
    let stamp = record.get("timestamp")?.as_str()?;
    let day = stamp.get(..10)?;
    let shaped = day.len() == 10
        && day.bytes().enumerate().all(|(at, byte)| {
            if at == 4 || at == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        });
    shaped.then(|| day.to_owned())
}

fn delta(now: TokenUsage, before: TokenUsage) -> TokenUsage {
    TokenUsage {
        input: now.input.saturating_sub(before.input),
        output: now.output.saturating_sub(before.output),
        cache_read: now.cache_read.saturating_sub(before.cache_read),
        cache_write: now.cache_write.saturating_sub(before.cache_write),
    }
}

fn is_empty(usage: TokenUsage) -> bool {
    usage == TokenUsage::default()
}

/// Reads the complete lines after `state.offset`, at most [`MAX_READ_BYTES`], and returns them with
/// the number of bytes the offset moved. A line longer than one read is skipped through its newline,
/// so the scan never stalls on it and never parses its tail. A shrunken file starts over.
fn new_lines(path: &Path, state: &mut FileState) -> io::Result<(Vec<u8>, u64)> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() < state.offset {
        *state = FileState::default();
    }
    file.seek(SeekFrom::Start(state.offset))?;
    let mut buffer = Vec::new();
    file.take(MAX_READ_BYTES as u64).read_to_end(&mut buffer)?;
    let mut from = 0;
    if state.skipping {
        let Some(at) = buffer.iter().position(|byte| *byte == b'\n') else {
            let skipped = buffer.len() as u64;
            state.offset += skipped;
            return Ok((Vec::new(), skipped));
        };
        from = at + 1;
        state.skipping = false;
    }
    let Some(end) = buffer[from..].iter().rposition(|byte| *byte == b'\n') else {
        let consumed = if from == 0 && buffer.len() >= MAX_READ_BYTES {
            state.skipping = true;
            buffer.len()
        } else {
            from
        };
        state.offset += consumed as u64;
        return Ok((Vec::new(), consumed as u64));
    };
    let consumed = from + end + 1;
    buffer.truncate(consumed);
    buffer.drain(..from);
    state.offset += consumed as u64;
    Ok((buffer, consumed as u64))
}

fn has(line: &[u8], needle: &[u8]) -> bool {
    line.windows(needle.len()).any(|window| window == needle)
}

fn finish(
    file: &LogFile,
    state: &FileState,
    days: BTreeMap<String, TokenUsage>,
    bytes: u64,
) -> Scanned {
    let buckets = days
        .into_iter()
        .filter(|(_, usage)| !is_empty(*usage))
        .map(|(day, usage)| Bucket {
            day,
            project: state.project.clone(),
            session: file.session.clone(),
            usage,
        })
        .collect();
    Scanned { buckets, bytes }
}

fn scan_claude(file: &LogFile, state: &mut FileState) -> io::Result<Scanned> {
    let (buffer, consumed) = new_lines(&file.path, state)?;
    let mut days: BTreeMap<String, TokenUsage> = BTreeMap::new();
    for line in buffer.split(|byte| *byte == b'\n') {
        if line.len() > MAX_LINE_BYTES || !has(line, b"\"usage\"") {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        if let Some(cwd) = record.get("cwd").and_then(Value::as_str) {
            if state.project.is_empty() {
                cwd.clone_into(&mut state.project);
            }
        }
        let Some(message) = record.get("message") else {
            continue;
        };
        let Some(usage) = message.get("usage") else {
            continue;
        };
        let now = TokenUsage {
            input: count(usage, "input_tokens"),
            output: count(usage, "output_tokens"),
            cache_read: count(usage, "cache_read_input_tokens"),
            cache_write: count(usage, "cache_creation_input_tokens"),
        };
        let id = message.get("id").and_then(Value::as_str).map(str::to_owned);
        let added = if id.is_some() && id == state.last_id {
            delta(now, state.last_usage)
        } else {
            now
        };
        state.last_id = id;
        state.last_usage = now;
        let Some(day) = day_of(&record) else {
            continue;
        };
        let slot = days.entry(day).or_default();
        slot.input += added.input;
        slot.output += added.output;
        slot.cache_read += added.cache_read;
        slot.cache_write += added.cache_write;
    }
    Ok(finish(file, state, days, consumed))
}

fn scan_codex(file: &LogFile, state: &mut FileState) -> io::Result<Scanned> {
    let (buffer, consumed) = new_lines(&file.path, state)?;
    let mut days: BTreeMap<String, TokenUsage> = BTreeMap::new();
    for line in buffer.split(|byte| *byte == b'\n') {
        let meta = has(line, b"session_meta");
        if line.len() > MAX_LINE_BYTES || !(meta || has(line, b"token_count")) {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let Some(payload) = record.get("payload") else {
            continue;
        };
        if meta && record.get("type").and_then(Value::as_str) == Some("session_meta") {
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                if state.project.is_empty() {
                    cwd.clone_into(&mut state.project);
                }
            }
            continue;
        }
        if payload.get("type").and_then(Value::as_str) != Some("token_count") {
            continue;
        }
        let Some(total) = payload.pointer("/info/total_token_usage") else {
            continue;
        };
        let cached = count(total, "cached_input_tokens");
        let now = TokenUsage {
            input: count(total, "input_tokens").saturating_sub(cached),
            output: count(total, "output_tokens"),
            cache_read: cached,
            cache_write: count(total, "cache_write_input_tokens"),
        };
        let added = delta(now, state.previous_total);
        state.previous_total = now;
        let Some(day) = day_of(&record) else {
            continue;
        };
        let slot = days.entry(day).or_default();
        slot.input += added.input;
        slot.output += added.output;
        slot.cache_read += added.cache_read;
        slot.cache_write += added.cache_write;
    }
    Ok(finish(file, state, days, consumed))
}

/// The first of `keys` that holds a count.
fn first_count(value: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| value.get(key).and_then(Value::as_u64))
        .unwrap_or(0)
}

/// Grok reports usage once per finished turn, so each record is added as it is, not differenced.
/// `input_tokens` is the uncached input; reasoning tokens count as output.
fn scan_grok(file: &LogFile, state: &mut FileState) -> io::Result<Scanned> {
    let (buffer, consumed) = new_lines(&file.path, state)?;
    let mut days: BTreeMap<String, TokenUsage> = BTreeMap::new();
    for line in buffer.split(|byte| *byte == b'\n') {
        if line.len() > MAX_LINE_BYTES || !has(line, b"turn_completed") {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let body = record
            .get("update")
            .or_else(|| record.get("payload"))
            .unwrap_or(&record);
        let kind = ["type", "event", "kind", "sessionUpdate"]
            .iter()
            .find_map(|key| {
                body.get(key)
                    .or_else(|| record.get(key))
                    .and_then(Value::as_str)
            });
        if kind != Some("turn_completed") {
            continue;
        }
        if let Some(cwd) = body
            .get("cwd")
            .or_else(|| record.get("cwd"))
            .and_then(Value::as_str)
        {
            if state.project.is_empty() {
                cwd.clone_into(&mut state.project);
            }
        }
        let Some(usage) = body.get("usage").or_else(|| record.get("usage")) else {
            continue;
        };
        let added = TokenUsage {
            input: count(usage, "input_tokens"),
            output: count(usage, "output_tokens") + count(usage, "reasoning_tokens"),
            cache_read: first_count(
                usage,
                &[
                    "cache_read_tokens",
                    "cached_input_tokens",
                    "cache_read_input_tokens",
                ],
            ),
            cache_write: first_count(
                usage,
                &["cache_write_tokens", "cache_creation_input_tokens"],
            ),
        };
        let Some(day) = day_of(body).or_else(|| day_of(&record)) else {
            continue;
        };
        let slot = days.entry(day).or_default();
        slot.input += added.input;
        slot.output += added.output;
        slot.cache_read += added.cache_read;
        slot.cache_write += added.cache_write;
    }
    Ok(finish(file, state, days, consumed))
}

fn grok_session(path: &Path) -> Option<String> {
    if path.file_name().and_then(|name| name.to_str()) != Some("updates.jsonl") {
        return None;
    }
    path.parent()?.file_name()?.to_str().map(str::to_owned)
}

/// Grok session logs under `sessions`.
#[must_use]
pub fn grok_logs_in(sessions: &Path) -> Vec<LogFile> {
    let mut found = Vec::new();
    collect_logs(sessions, 0, &grok_session, &mut found);
    newest_first(found)
}

fn newest_first(mut files: Vec<(std::time::SystemTime, LogFile)>) -> Vec<LogFile> {
    files.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    files.truncate(MAX_LOG_FILES);
    files.into_iter().map(|(_, file)| file).collect()
}

/// Logs of Claude Code under `projects`, one folder per working directory.
#[must_use]
pub fn claude_logs_in(projects: &Path) -> Vec<LogFile> {
    let mut found = Vec::new();
    let Ok(folders) = fs::read_dir(projects) else {
        return Vec::new();
    };
    for folder in folders.flatten() {
        let Ok(entries) = fs::read_dir(folder.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(session) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
                continue;
            };
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            found.push((modified, LogFile { path, session }));
        }
    }
    newest_first(found)
}

/// Walks `folder` for files `session_of` accepts, giving each the session it returns.
fn collect_logs(
    folder: &Path,
    depth: usize,
    session_of: &dyn Fn(&Path) -> Option<String>,
    found: &mut Vec<(std::time::SystemTime, LogFile)>,
) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth < 4 {
                collect_logs(&path, depth + 1, session_of, found);
            }
            continue;
        }
        let Some(session) = session_of(&path) else {
            continue;
        };
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        found.push((modified, LogFile { path, session }));
    }
}

fn rollout_session(path: &Path) -> Option<String> {
    let stem = path.file_stem().and_then(|s| s.to_str())?;
    if !stem.starts_with("rollout-") || path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return None;
    }
    Some(
        stem.get(stem.len().saturating_sub(36)..)
            .unwrap_or(stem)
            .to_owned(),
    )
}

/// Rollout logs of Codex under `sessions`.
#[must_use]
pub fn codex_logs_in(sessions: &Path) -> Vec<LogFile> {
    let mut found = Vec::new();
    collect_logs(sessions, 0, &rollout_session, &mut found);
    newest_first(found)
}

impl UsageProvider for ClaudeProvider {
    fn id(&self) -> &'static str {
        "claude_code"
    }
    fn display_name(&self) -> &'static str {
        "Claude Code"
    }
    fn icon(&self) -> &'static str {
        "claude"
    }
    fn log_files(&self) -> Vec<LogFile> {
        home_dir("CLAUDE_CONFIG_DIR", ".claude")
            .map_or_else(Vec::new, |dir| claude_logs_in(&dir.join("projects")))
    }
    fn scan(&self, file: &LogFile, state: &mut FileState) -> io::Result<Scanned> {
        scan_claude(file, state)
    }
}

impl UsageProvider for CodexProvider {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn display_name(&self) -> &'static str {
        "Codex"
    }
    fn icon(&self) -> &'static str {
        "codex"
    }
    fn log_files(&self) -> Vec<LogFile> {
        home_dir("CODEX_HOME", ".codex")
            .map_or_else(Vec::new, |dir| codex_logs_in(&dir.join("sessions")))
    }
    fn scan(&self, file: &LogFile, state: &mut FileState) -> io::Result<Scanned> {
        scan_codex(file, state)
    }
}

impl UsageProvider for GrokProvider {
    fn id(&self) -> &'static str {
        "grok"
    }
    fn display_name(&self) -> &'static str {
        "Grok"
    }
    fn icon(&self) -> &'static str {
        "grok"
    }
    fn log_files(&self) -> Vec<LogFile> {
        home_dir("GROK_HOME", ".grok")
            .map_or_else(Vec::new, |dir| grok_logs_in(&dir.join("sessions")))
    }
    fn scan(&self, file: &LogFile, state: &mut FileState) -> io::Result<Scanned> {
        scan_grok(file, state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, text: &str) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open");
        file.write_all(text.as_bytes()).expect("write");
    }

    fn claude_line(id: &str, day: &str, output: u64) -> String {
        format!(
            r#"{{"timestamp":"{day}T10:00:00Z","cwd":"/work/a","message":{{"id":"{id}","usage":{{"input_tokens":1,"output_tokens":{output},"cache_read_input_tokens":10,"cache_creation_input_tokens":0}}}}}}"#
        )
    }

    fn file_in(dir: &Path) -> LogFile {
        LogFile {
            path: dir.join("s.jsonl"),
            session: "s".to_owned(),
        }
    }

    #[test]
    fn claude_usage_is_bucketed_by_day_and_a_repeated_message_counts_once() {
        let dir = tempfile::tempdir().expect("dir");
        let file = file_in(dir.path());
        write(
            &file.path,
            &format!(
                "{}\n{}\n{}\n",
                claude_line("m1", "2026-11-01", 5),
                claude_line("m1", "2026-11-01", 5),
                claude_line("m2", "2026-11-02", 7)
            ),
        );
        let scanned = scan_claude(&file, &mut FileState::default()).expect("scan");
        let days: Vec<_> = scanned
            .buckets
            .iter()
            .map(|b| (b.day.as_str(), b.usage.output, b.project.as_str()))
            .collect();
        assert_eq!(
            days,
            [("2026-11-01", 5, "/work/a"), ("2026-11-02", 7, "/work/a")]
        );
    }

    #[test]
    fn a_second_scan_adds_only_what_was_appended() {
        let dir = tempfile::tempdir().expect("dir");
        let file = file_in(dir.path());
        let mut state = FileState::default();
        write(
            &file.path,
            &format!("{}\n", claude_line("m1", "2026-11-01", 5)),
        );
        scan_claude(&file, &mut state).expect("scan");
        write(
            &file.path,
            &format!("broken \"usage\"\n{}\n", claude_line("m2", "2026-11-01", 3)),
        );
        let again = scan_claude(&file, &mut state).expect("scan");
        assert_eq!(again.buckets.len(), 1);
        assert_eq!(again.buckets[0].usage.output, 3);
        assert!(scan_claude(&file, &mut state)
            .expect("scan")
            .buckets
            .is_empty());
    }

    #[test]
    fn codex_cumulative_totals_become_daily_differences() {
        let dir = tempfile::tempdir().expect("dir");
        let file = file_in(dir.path());
        let meta = r#"{"timestamp":"2026-11-01T01:00:00Z","type":"session_meta","payload":{"id":"x","cwd":"/work/b"}}"#;
        let event = |day: &str, input: u64, output: u64| {
            format!(
                r#"{{"timestamp":"{day}T02:00:00Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"cached_input_tokens":0,"output_tokens":{output}}}}}}}}}"#
            )
        };
        write(
            &file.path,
            &format!(
                "{meta}\n{}\n{}\n",
                event("2026-11-01", 100, 10),
                event("2026-11-02", 250, 30)
            ),
        );
        let scanned = scan_codex(&file, &mut FileState::default()).expect("scan");
        let found: Vec<_> = scanned
            .buckets
            .iter()
            .map(|b| {
                (
                    b.day.as_str(),
                    b.usage.input,
                    b.usage.output,
                    b.project.as_str(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("2026-11-01", 100, 10, "/work/b"),
                ("2026-11-02", 150, 20, "/work/b")
            ]
        );
    }

    #[test]
    fn an_oversized_record_is_skipped_through_its_newline_and_later_usage_counts_once() {
        let dir = tempfile::tempdir().expect("dir");
        let file = file_in(dir.path());
        // The part after the first read chunk is valid usage JSON; it must not be counted.
        let oversized = format!(
            "{}{}\n",
            "x".repeat(MAX_READ_BYTES),
            claude_line("ghost", "2026-11-01", 100)
        );
        write(&file.path, &oversized);
        write(
            &file.path,
            &format!("{}\n", claude_line("m1", "2026-11-01", 7)),
        );
        let mut saved: Option<String> = None;
        let mut output = 0;
        for _ in 0..4 {
            let mut state = saved
                .as_deref()
                .and_then(|text| serde_json::from_str::<FileState>(text).ok())
                .unwrap_or_default();
            let scanned = scan_claude(&file, &mut state).expect("scan");
            output += scanned.buckets.iter().map(|b| b.usage.output).sum::<u64>();
            // The app saves the state only when a scan reports progress.
            if scanned.bytes > 0 {
                saved = Some(serde_json::to_string(&state).expect("state"));
            }
        }
        assert_eq!(output, 7);
        let state: FileState =
            serde_json::from_str(saved.as_deref().expect("saved")).expect("state");
        assert_eq!(state.offset, fs::metadata(&file.path).expect("len").len());
    }

    #[test]
    fn a_state_saved_before_skipping_existed_still_loads() {
        let mut old = serde_json::to_value(FileState {
            offset: 5,
            ..FileState::default()
        })
        .expect("state");
        old.as_object_mut().expect("object").remove("skipping");
        let state: FileState = serde_json::from_value(old).expect("old state");
        assert_eq!(state.offset, 5);
        assert!(!state.skipping);
    }

    #[test]
    fn logs_are_found_and_a_missing_folder_gives_none() {
        let projects = tempfile::tempdir().expect("dir");
        let folder = projects.path().join("proj");
        fs::create_dir_all(&folder).expect("folder");
        write(&folder.join("abc.jsonl"), "{}\n");
        write(&folder.join("note.txt"), "x");
        let logs = claude_logs_in(projects.path());
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].session, "abc");
        assert!(claude_logs_in(&projects.path().join("none")).is_empty());
    }

    #[test]
    fn a_line_with_a_bad_day_is_not_counted() {
        let dir = tempfile::tempdir().expect("dir");
        let file = file_in(dir.path());
        write(
            &file.path,
            &format!("{}\n", claude_line("m1", "not-a-date", 5)),
        );
        assert!(scan_claude(&file, &mut FileState::default())
            .expect("scan")
            .buckets
            .is_empty());
    }

    #[test]
    fn grok_turns_add_up_per_day_with_reasoning_as_output() {
        let dir = tempfile::tempdir().expect("dir");
        let file = file_in(dir.path());
        let turn = |day: &str, input: u64, output: u64, reasoning: u64, cache: u64| {
            format!(
                r#"{{"timestamp":"{day}T09:00:00Z","update":{{"type":"turn_completed","cwd":"/work/g","usage":{{"input_tokens":{input},"output_tokens":{output},"reasoning_tokens":{reasoning},"cache_read_tokens":{cache}}}}}}}"#
            )
        };
        write(
            &file.path,
            &format!(
                "{}\n{}\n{}\n{}\n",
                turn("2026-11-01", 100, 10, 5, 40),
                turn("2026-11-01", 20, 2, 0, 0),
                r#"{"type":"other"}"#,
                turn("2026-11-02", 7, 1, 1, 3)
            ),
        );
        let mut state = FileState::default();
        let scanned = scan_grok(&file, &mut state).expect("scan");
        let found: Vec<_> = scanned
            .buckets
            .iter()
            .map(|b| {
                (
                    b.day.as_str(),
                    b.usage.input,
                    b.usage.output,
                    b.usage.cache_read,
                )
            })
            .collect();
        assert_eq!(
            found,
            [("2026-11-01", 120, 17, 40), ("2026-11-02", 7, 2, 3)]
        );
        assert_eq!(scanned.buckets[0].project, "/work/g");
        assert!(scan_grok(&file, &mut state)
            .expect("scan")
            .buckets
            .is_empty());
    }

    #[test]
    fn grok_logs_are_found_by_name_and_a_missing_folder_gives_none() {
        let sessions = tempfile::tempdir().expect("dir");
        let folder = sessions.path().join("2026").join("abc123");
        fs::create_dir_all(&folder).expect("folder");
        write(&folder.join("updates.jsonl"), "{}\n");
        write(&folder.join("other.jsonl"), "{}\n");
        let logs = grok_logs_in(sessions.path());
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].session, "abc123");
        assert!(grok_logs_in(&sessions.path().join("none")).is_empty());
    }
}
