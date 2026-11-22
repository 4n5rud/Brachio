//! Token totals read from the agents' own local session logs. Nothing leaves the machine.
//!
//! Claude Code writes `~/.claude/projects/<encoded cwd>/<session id>.jsonl`, one JSON object per
//! line, and an assistant message carries `message.usage`. The same message can appear on several
//! lines (one per content block) with the same id and usage, so only the last line of a run of one
//! id counts. Codex writes `rollout-*.jsonl` with `token_count` events whose
//! `total_token_usage` is already cumulative, so the newest one is the total.
//!
//! A [`UsageTracker`] remembers the byte offset it stopped at and reads only what was appended.

use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The most bytes one refresh reads, so a huge log is caught up over several refreshes.
pub const MAX_READ_BYTES: usize = 8 * 1024 * 1024;
/// A line longer than this is skipped without being parsed.
pub const MAX_LINE_BYTES: usize = 2 * 1024 * 1024;

/// Token counts of one session. Cache reads and writes are kept apart from plain input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Input tokens that were not served from the cache.
    pub input: u64,
    /// Output tokens, including reasoning.
    pub output: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the prompt cache.
    pub cache_write: u64,
}

impl TokenUsage {
    /// Everything the session processed.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    fn add(&mut self, other: Self) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
    }

    fn remove(&mut self, other: Self) {
        self.input = self.input.saturating_sub(other.input);
        self.output = self.output.saturating_sub(other.output);
        self.cache_read = self.cache_read.saturating_sub(other.cache_read);
        self.cache_write = self.cache_write.saturating_sub(other.cache_write);
    }
}

/// Which agent wrote the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Claude Code project log.
    Claude,
    /// Codex rollout log.
    Codex,
}

#[derive(Debug, Default)]
struct Running {
    total: TokenUsage,
    last_id: Option<String>,
    last_usage: TokenUsage,
}

fn count(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn apply_claude(running: &mut Running, record: &Value) {
    let Some(message) = record.get("message") else {
        return;
    };
    let Some(usage) = message.get("usage") else {
        return;
    };
    let now = TokenUsage {
        input: count(usage, "input_tokens"),
        output: count(usage, "output_tokens"),
        cache_read: count(usage, "cache_read_input_tokens"),
        cache_write: count(usage, "cache_creation_input_tokens"),
    };
    let id = message.get("id").and_then(Value::as_str).map(str::to_owned);
    if id.is_some() && id == running.last_id {
        running.total.remove(running.last_usage);
    }
    running.total.add(now);
    running.last_id = id;
    running.last_usage = now;
}

fn apply_codex(running: &mut Running, record: &Value) {
    if record.get("type").and_then(Value::as_str) != Some("event_msg") {
        return;
    }
    let Some(payload) = record.get("payload") else {
        return;
    };
    if payload.get("type").and_then(Value::as_str) != Some("token_count") {
        return;
    }
    let Some(total) = payload.pointer("/info/total_token_usage") else {
        return;
    };
    let input_with_cache = count(total, "input_tokens");
    let cached = count(total, "cached_input_tokens");
    running.total = TokenUsage {
        input: input_with_cache.saturating_sub(cached),
        output: count(total, "output_tokens"),
        cache_read: cached,
        cache_write: count(total, "cache_write_input_tokens"),
    };
}

/// Feeds one line; a line that is not JSON, or has no usage, changes nothing.
fn apply_line(format: LogFormat, running: &mut Running, line: &[u8]) {
    let needle: &[u8] = match format {
        LogFormat::Claude => b"\"usage\"",
        LogFormat::Codex => b"token_count",
    };
    if line.len() > MAX_LINE_BYTES || !line.windows(needle.len()).any(|window| window == needle) {
        return;
    }
    let Ok(record) = serde_json::from_slice::<Value>(line) else {
        return;
    };
    match format {
        LogFormat::Claude => apply_claude(running, &record),
        LogFormat::Codex => apply_codex(running, &record),
    }
}

/// Reads one log incrementally. Only complete lines are counted, so a line still being written is
/// picked up whole on the next refresh.
#[derive(Debug)]
pub struct UsageTracker {
    format: LogFormat,
    path: PathBuf,
    offset: u64,
    running: Running,
}

impl UsageTracker {
    /// A tracker positioned at the start of `path`.
    #[must_use]
    pub fn new(format: LogFormat, path: PathBuf) -> Self {
        Self {
            format,
            path,
            offset: 0,
            running: Running::default(),
        }
    }

    /// The log this tracker reads.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads what was appended since the last call, at most [`MAX_READ_BYTES`], and returns the totals.
    ///
    /// # Errors
    /// Fails when the file cannot be opened or read.
    pub fn refresh(&mut self) -> io::Result<TokenUsage> {
        let mut file = File::open(&self.path)?;
        let length = file.metadata()?.len();
        if length < self.offset {
            self.offset = 0;
            self.running = Running::default();
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buffer = Vec::new();
        file.take(MAX_READ_BYTES as u64).read_to_end(&mut buffer)?;
        let Some(end) = buffer.iter().rposition(|byte| *byte == b'\n') else {
            // No complete line yet; a line longer than the cap is skipped so the reader cannot stall.
            if buffer.len() >= MAX_READ_BYTES {
                self.offset += buffer.len() as u64;
            }
            return Ok(self.running.total);
        };
        for line in buffer[..end].split(|byte| *byte == b'\n') {
            apply_line(self.format, &mut self.running, line);
        }
        self.offset += end as u64 + 1;
        Ok(self.running.total)
    }
}

fn claude_config_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".claude"))
}

/// Claude Code names a project folder after the working directory with every character that is
/// not a letter or digit replaced by `-`.
#[must_use]
pub fn encode_claude_cwd(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The Claude Code log of a task: its session file when the id is known and present, otherwise the
/// newest log in the working directory's project folder.
#[must_use]
pub fn claude_log_path(cwd: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    claude_log_in(&claude_config_dir()?.join("projects"), cwd, session_id)
}

/// Same as [`claude_log_path`], looking in `projects` instead of the default folder.
#[must_use]
pub fn claude_log_in(projects: &Path, cwd: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    let folder = projects.join(encode_claude_cwd(cwd));
    if let Some(id) = session_id {
        let own = folder.join(format!("{id}.jsonl"));
        if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && own.is_file() {
            return Some(own);
        }
    }
    newest_jsonl(&folder)
}

fn newest_jsonl(folder: &Path) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(folder).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FIRST: &str = r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":2,"output_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":50}}}"#;
    const REPEAT: &str = r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":2,"output_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":50}}}"#;
    const SECOND: &str = r#"{"type":"assistant","message":{"id":"m2","usage":{"input_tokens":3,"output_tokens":7,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#;

    fn append(path: &Path, text: &str) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open");
        file.write_all(text.as_bytes()).expect("write");
    }

    #[test]
    fn a_message_repeated_across_lines_counts_once() {
        let dir = tempfile::tempdir().expect("dir");
        let log = dir.path().join("s.jsonl");
        append(&log, &format!("{FIRST}\n{REPEAT}\n{SECOND}\n"));
        let usage = UsageTracker::new(LogFormat::Claude, log)
            .refresh()
            .expect("read");
        assert_eq!(
            usage,
            TokenUsage {
                input: 5,
                output: 17,
                cache_read: 100,
                cache_write: 50
            }
        );
        assert_eq!(usage.total(), 172);
    }

    #[test]
    fn malformed_and_unrelated_lines_are_skipped() {
        let dir = tempfile::tempdir().expect("dir");
        let log = dir.path().join("s.jsonl");
        append(&log, &format!("not json \"usage\"\n{{\"type\":\"user\"}}\n{{\"message\":{{\"usage\":\"x\"}}}}\n{SECOND}\n"));
        let usage = UsageTracker::new(LogFormat::Claude, log)
            .refresh()
            .expect("read");
        assert_eq!(usage.output, 7);
    }

    #[test]
    fn only_appended_complete_lines_are_read_next_time() {
        let dir = tempfile::tempdir().expect("dir");
        let log = dir.path().join("s.jsonl");
        append(&log, &format!("{FIRST}\n{}", &SECOND[..40]));
        let mut tracker = UsageTracker::new(LogFormat::Claude, log.clone());
        assert_eq!(tracker.refresh().expect("read").output, 10);
        append(&log, &format!("{}\n", &SECOND[40..]));
        assert_eq!(tracker.refresh().expect("read").output, 17);
    }

    #[test]
    fn a_truncated_log_is_read_again_from_the_start() {
        let dir = tempfile::tempdir().expect("dir");
        let log = dir.path().join("s.jsonl");
        append(&log, &format!("{FIRST}\n{SECOND}\n"));
        let mut tracker = UsageTracker::new(LogFormat::Claude, log.clone());
        tracker.refresh().expect("read");
        fs::write(&log, format!("{SECOND}\n")).expect("rewrite");
        assert_eq!(tracker.refresh().expect("read").output, 7);
    }

    #[test]
    fn codex_uses_the_newest_cumulative_total() {
        let dir = tempfile::tempdir().expect("dir");
        let log = dir.path().join("rollout.jsonl");
        let event = |input: u64, cached: u64, output: u64| {
            format!(
                r#"{{"type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"cache_write_input_tokens":0,"output_tokens":{output},"total_tokens":{}}}}}}}}}"#,
                input + output
            )
        };
        append(&log, &format!("{}\n{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":null}}}}\n{}\n", event(100, 40, 5), event(300, 200, 20)));
        let usage = UsageTracker::new(LogFormat::Codex, log)
            .refresh()
            .expect("read");
        assert_eq!(
            usage,
            TokenUsage {
                input: 100,
                output: 20,
                cache_read: 200,
                cache_write: 0
            }
        );
    }

    #[test]
    fn a_missing_log_is_an_error_not_zero() {
        let dir = tempfile::tempdir().expect("dir");
        assert!(
            UsageTracker::new(LogFormat::Claude, dir.path().join("none.jsonl"))
                .refresh()
                .is_err()
        );
    }

    #[test]
    fn the_claude_folder_name_replaces_every_symbol() {
        assert_eq!(
            encode_claude_cwd(Path::new("C:\\Users\\me\\a.b_c")),
            "C--Users-me-a-b-c"
        );
    }

    #[test]
    fn the_session_file_is_preferred_then_the_newest_log() {
        let projects = tempfile::tempdir().expect("dir");
        let cwd = Path::new("/work/tree");
        let folder = projects.path().join(encode_claude_cwd(cwd));
        fs::create_dir_all(&folder).expect("folder");
        append(&folder.join("old.jsonl"), "{}\n");
        append(&folder.join("wanted.jsonl"), "{}\n");
        assert_eq!(
            claude_log_in(projects.path(), cwd, Some("wanted")),
            Some(folder.join("wanted.jsonl"))
        );
        assert!(claude_log_in(projects.path(), cwd, Some("absent")).is_some());
        assert!(claude_log_in(projects.path(), Path::new("/other"), None).is_none());
        assert!(claude_log_in(projects.path(), cwd, Some("../escape")).is_some());
    }
}
