//! One running program: its output, screen and what it reported about itself.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use brachio_pty::SessionId;
use brachio_runtime::wire::{
    LostInfo, OutputPage, ResponseResult, ScreenSnapshot, SessionInfo, StatusInfo,
};
use tokio::sync::watch;

use crate::journal::SessionRecord;
use crate::output::{OutputHistory, Page};
use crate::screen::TerminalScreen;

/// The cursor position query that a terminal program sends at startup; the runtime answers it
/// so the program can start without a screen.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";
/// The newer ConPTY passes the device attributes query through and waits about 3 s for an answer.
const DEVICE_QUERY: &[u8] = b"\x1b[c";
const DEVICE_REPLY: &[u8] = b"\x1b[?61;4c";

#[derive(Clone)]
pub(crate) struct Session {
    pub(crate) id: SessionId,
    pub(crate) pid: u32,
    pub(crate) cwd: PathBuf,
    pub(crate) output: Arc<Mutex<StartupOutput>>,
    pub(crate) updates: watch::Receiver<()>,
}

impl Session {
    pub(crate) fn describe(&self, after: Option<u64>) -> Result<ResponseResult> {
        let state = lock(&self.output)?;
        let output = after
            .map(|offset| {
                state
                    .history
                    .read(offset)
                    .map(output_page)
                    .context("output offset is past the end")
            })
            .transpose()?;
        Ok(ResponseResult::Success {
            session: self.info(&state),
            output,
        })
    }

    /// What decides whether the saved record is out of date, without formatting the screen.
    pub(crate) fn mark(&self) -> Result<(u64, bool)> {
        let state = lock(&self.output)?;
        Ok((state.history.end(), state.exited))
    }

    pub(crate) fn record(&self, key: &str, instance: &str) -> Result<SessionRecord> {
        let state = lock(&self.output)?;
        let frame = state.screen.frame();
        let saved_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        Ok(SessionRecord {
            key: key.to_owned(),
            cwd: self.cwd.clone(),
            instance: instance.to_owned(),
            pid: self.pid,
            rows: frame.rows,
            cols: frame.cols,
            ansi: frame.ansi,
            cursor: frame.cursor,
            alternate_screen: frame.alternate_screen,
            output_bytes: state.history.end(),
            exited: state.exited,
            exit_code: state.exit_code,
            saved_at_ms,
        })
    }

    pub(crate) fn snapshot(&self) -> Result<ResponseResult> {
        let state = lock(&self.output)?;
        let frame = state.screen.frame();
        let resume_offset =
            (frame.at_boundary && !frame.alternate_screen).then(|| state.history.end());
        Ok(ResponseResult::Snapshot {
            session: self.info(&state),
            screen: ScreenSnapshot {
                ansi: frame.ansi,
                cursor: frame.cursor,
                rows: frame.rows,
                cols: frame.cols,
                alternate_screen: frame.alternate_screen,
                resume_offset,
            },
        })
    }

    pub(crate) fn info(&self, state: &StartupOutput) -> SessionInfo {
        SessionInfo {
            session_id: self.id,
            pid: self.pid,
            output_bytes: state.history.end(),
            exited: state.exited,
            exit_code: state.exit_code,
            status: state.status.clone(),
        }
    }
}

#[derive(Default)]
pub(crate) struct StartupOutput {
    pub(crate) status: Option<StatusInfo>,
    pub(crate) id: Option<SessionId>,
    pub(crate) history: OutputHistory,
    pub(crate) screen: TerminalScreen,
    cursor: StartupQuery,
    device: StartupQuery,
    pub(crate) exited: bool,
    pub(crate) exit_code: Option<u32>,
}

/// Finds the first occurrence of one query in the output, even when it is split across chunks.
#[derive(Default)]
struct StartupQuery {
    matched: usize,
    answered: bool,
}

impl StartupQuery {
    fn scan(&mut self, query: &[u8], bytes: &[u8]) {
        if self.answered || self.matched == query.len() {
            return;
        }
        for &byte in bytes {
            if byte == query[self.matched] {
                self.matched += 1;
                if self.matched == query.len() {
                    break;
                }
            } else {
                self.matched = usize::from(byte == query[0]);
            }
        }
    }

    fn take_reply<'a>(&mut self, query: &[u8], reply: &'a [u8]) -> Option<&'a [u8]> {
        if self.answered || self.matched != query.len() {
            return None;
        }
        self.answered = true;
        Some(reply)
    }
}

impl StartupOutput {
    pub(crate) fn record(&mut self, bytes: &[u8]) -> Option<(SessionId, Vec<u8>)> {
        self.history.push(bytes);
        self.screen.process(bytes);
        self.cursor.scan(CURSOR_QUERY, bytes);
        self.device.scan(DEVICE_QUERY, bytes);
        self.reply_if_needed()
    }

    /// The session and the bytes that answer the startup queries seen so far.
    pub(crate) fn reply_if_needed(&mut self) -> Option<(SessionId, Vec<u8>)> {
        let id = self.id?;
        let mut reply = Vec::new();
        if let Some(bytes) = self.cursor.take_reply(CURSOR_QUERY, CURSOR_REPLY) {
            reply.extend_from_slice(bytes);
        }
        if let Some(bytes) = self.device.take_reply(DEVICE_QUERY, DEVICE_REPLY) {
            reply.extend_from_slice(bytes);
        }
        (!reply.is_empty()).then_some((id, reply))
    }
}

pub(crate) fn lost_info(record: &SessionRecord) -> LostInfo {
    LostInfo {
        key: record.key.clone(),
        cwd: record.cwd.clone(),
        former_instance: record.instance.clone(),
        former_pid: record.pid,
        exited: record.exited,
        exit_code: record.exit_code,
        output_bytes: record.output_bytes,
        saved_at_ms: record.saved_at_ms,
    }
}

pub(crate) fn output_page(page: Page) -> OutputPage {
    OutputPage {
        start: page.start,
        next: page.next,
        end: page.end,
        truncated: page.truncated,
        bytes: page.bytes,
    }
}
pub(crate) fn get<'a>(sessions: &'a HashMap<String, Session>, key: &str) -> Result<&'a Session> {
    sessions
        .get(key)
        .with_context(|| format!("no active terminal for {key}"))
}

pub(crate) fn lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>> {
    value
        .lock()
        .map_err(|_| anyhow!("runtime state lock poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_queries_are_each_answered_once_even_when_split() {
        let mut output = StartupOutput {
            id: serde_json::from_str("1").ok(),
            ..StartupOutput::default()
        };
        let mut replies = Vec::new();
        for chunk in [&b"hi\x1b[6"[..], b"n\x1b", b"[c more\x1b[6n\x1b[c"] {
            if let Some((_, bytes)) = output.record(chunk) {
                replies.push(bytes);
            }
        }
        assert_eq!(replies, [b"\x1b[1;1R".to_vec(), b"\x1b[?61;4c".to_vec()]);
    }
}
