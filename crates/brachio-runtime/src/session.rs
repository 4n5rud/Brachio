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

/// The cursor position query that a terminal program sends at startup; the runtime answers it.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";

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
    pub(crate) matched: usize,
    pub(crate) answered: bool,
    pub(crate) exited: bool,
    pub(crate) exit_code: Option<u32>,
}

impl StartupOutput {
    pub(crate) fn record(&mut self, bytes: &[u8]) -> Option<SessionId> {
        self.history.push(bytes);
        self.screen.process(bytes);
        if !self.answered && self.matched < CURSOR_QUERY.len() {
            for &byte in bytes {
                if byte == CURSOR_QUERY[self.matched] {
                    self.matched += 1;
                    if self.matched == CURSOR_QUERY.len() {
                        break;
                    }
                } else {
                    self.matched = usize::from(byte == CURSOR_QUERY[0]);
                }
            }
        }
        self.reply_if_needed()
    }

    pub(crate) fn reply_if_needed(&mut self) -> Option<SessionId> {
        if self.answered || self.matched != CURSOR_QUERY.len() {
            return None;
        }
        let id = self.id?;
        self.answered = true;
        Some(id)
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
