//! Messages exchanged between the runtime and its clients, one JSON object per line.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use brachio_pty::SessionId;
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::protocol;

/// Largest request a client may send.
pub const MAX_MESSAGE: u64 = 64 * 1024;
/// Largest response a runtime may send.
pub const MAX_RESPONSE: u64 = 256 * 1024;
/// How long one request or response may take to cross the connection.
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// One thing a client asks a runtime to do.
#[derive(Subcommand, Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Start one shell for this key, or return the existing one.
    Ensure {
        /// Name the caller chose for the shell.
        key: String,
        /// Directory the shell starts in.
        cwd: PathBuf,
    },
    /// Read the shell identity and number of PTY bytes received.
    Inspect {
        /// Name the caller chose for the shell.
        key: String,
    },
    /// Read a page of output starting at the requested absolute byte offset.
    Read {
        /// Name the caller chose for the shell.
        key: String,
        /// First byte offset to return.
        after: u64,
    },
    /// Replay and then follow output on the same connection.
    Follow {
        /// Name the caller chose for the shell.
        key: String,
        /// First byte offset to return.
        after: u64,
    },
    /// Capture the current visible frame of a running shell.
    Snapshot {
        /// Name the caller chose for the shell.
        key: String,
    },
    /// Send a settled normal-screen frame, then follow future output.
    FollowFrame {
        /// Name the caller chose for the shell.
        key: String,
    },
    /// Read the protocol range and drain state of this runtime.
    Hello,
    /// List shells that ended or were left by an earlier runtime.
    Lost,
    /// Read the last screen of a shell listed by `lost`.
    Recover {
        /// Name the caller chose for the shell.
        key: String,
    },
    /// Drop the saved screen of a shell listed by `lost`.
    Forget {
        /// Name the caller chose for the shell.
        key: String,
    },
    /// Take no new terminals and exit once the last one ends.
    Drain,
    /// Resize the shell and the frame that snapshots describe.
    Resize {
        /// Name the caller chose for the shell.
        key: String,
        /// New height in rows.
        rows: u16,
        /// New width in columns.
        cols: u16,
    },
    /// Write text to the shell.
    Write {
        /// Name the caller chose for the shell.
        key: String,
        /// Text to send, including any line ending.
        text: String,
    },
    /// Stop one shell explicitly.
    Stop {
        /// Name the caller chose for the shell.
        key: String,
    },
}

impl Action {
    /// The first protocol version that defines this action, and a name for error messages.
    #[must_use]
    pub fn since(&self) -> (u8, &'static str) {
        match self {
            Action::Hello => (2, "hello"),
            Action::Drain => (2, "drain"),
            Action::Resize { .. } => (2, "resize"),
            Action::Lost | Action::Recover { .. } | Action::Forget { .. } => (3, "recovery"),
            _ => (1, "this action"),
        }
    }
}

/// An action together with the protocol version the client speaks.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Request {
    /// Protocol version of this request.
    pub version: u8,
    /// What to do.
    pub action: Action,
}

/// What a runtime sends back, as the runtime writes it.
#[derive(Serialize)]
pub struct Response<'a> {
    /// Protocol version the reply is written in, echoing the request.
    pub version: u8,
    /// Which runtime answered, so a client can tell a restarted one.
    pub instance: &'a str,
    /// The outcome.
    #[serde(flatten)]
    pub result: ResponseResult,
}

/// What a runtime sends back, as a client reads it.
#[derive(Deserialize, Clone, Debug)]
pub struct Reply {
    /// Protocol version the reply is written in.
    pub version: u8,
    /// Which runtime answered.
    pub instance: String,
    /// The outcome.
    #[serde(flatten)]
    pub result: ResponseResult,
}

/// The outcome of one request. Variants are tried in order when reading, so the ones with
/// more fields come first.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ResponseResult {
    /// A running shell and its visible frame.
    Snapshot {
        /// The shell.
        session: SessionInfo,
        /// Its visible frame.
        screen: ScreenSnapshot,
    },
    /// A running shell and, when asked, a page of its output.
    Success {
        /// The shell.
        session: SessionInfo,
        /// Output starting at the requested offset.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<OutputPage>,
    },
    /// Version range and drain state of a runtime.
    Hello {
        /// The runtime.
        runtime: RuntimeInfo,
    },
    /// The last screen of a shell that is gone.
    Recovered {
        /// The shell that is gone.
        lost: LostInfo,
        /// Its last visible frame.
        screen: ScreenSnapshot,
    },
    /// Shells that are gone but can be recovered.
    Lost {
        /// The shells, ordered by key.
        lost: Vec<LostInfo>,
    },
    /// The request failed.
    Error {
        /// Why.
        error: String,
    },
}

/// A shell that is gone and whose last screen was saved.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LostInfo {
    /// The key the shell was started with.
    pub key: String,
    /// Its working directory.
    pub cwd: PathBuf,
    /// The runtime that owned it.
    pub former_instance: String,
    /// Its process ID, which is no longer alive.
    pub former_pid: u32,
    /// Whether the shell ended by itself.
    pub exited: bool,
    /// Its exit code, when it ended.
    pub exit_code: Option<u32>,
    /// Bytes of output it had produced when saved.
    pub output_bytes: u64,
    /// When the record was written, in milliseconds since the Unix epoch.
    pub saved_at_ms: u64,
}

/// Version range and drain state of a runtime.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RuntimeInfo {
    /// Process ID of the runtime.
    pub pid: u32,
    /// Oldest protocol version it answers.
    pub oldest_version: u8,
    /// Newest protocol version it speaks.
    pub current_version: u8,
    /// Whether it takes no new terminals.
    pub draining: bool,
}

/// A slice of a shell's output, addressed by absolute byte offsets.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OutputPage {
    /// First offset still held.
    pub start: u64,
    /// Offset to ask for next.
    pub next: u64,
    /// Offset after the last byte received.
    pub end: u64,
    /// Whether the requested start was older than the retained history.
    pub truncated: bool,
    /// The bytes from the requested offset.
    pub bytes: Vec<u8>,
}

/// Identity and progress of one running shell.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionInfo {
    /// Runtime-local session ID.
    pub session_id: SessionId,
    /// Process ID of the shell.
    pub pid: u32,
    /// Bytes received from the PTY so far.
    pub output_bytes: u64,
    /// Whether the shell has exited.
    pub exited: bool,
    /// Its exit code, when it exited.
    pub exit_code: Option<u32>,
}

/// The visible frame of a terminal, as escape sequences that redraw it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScreenSnapshot {
    /// Escape sequences that reproduce the frame.
    pub ansi: String,
    /// Cursor row and column.
    pub cursor: (u16, u16),
    /// Frame height.
    pub rows: u16,
    /// Frame width.
    pub cols: u16,
    /// Whether the alternate screen is active.
    pub alternate_screen: bool,
    /// Output offset from which live output continues this frame, when one exists.
    pub resume_offset: Option<u64>,
}

/// Check the requested version against what this build speaks and what the action needs.
///
/// # Errors
/// Returns a message naming the supported range or the version the action needs.
pub fn accepted(request: Request) -> Result<Request> {
    if !protocol::accepts(request.version) {
        bail!(protocol::rejection(request.version));
    }
    let (since, name) = request.action.since();
    protocol::requires(request.version, since, name).map_err(|message| anyhow!(message))?;
    Ok(request)
}

/// Read one newline-terminated message, or `None` when the peer closed the connection.
///
/// # Errors
/// Fails when the message is longer than `limit` or the connection breaks.
pub async fn read_message(
    reader: &mut (impl AsyncBufRead + Unpin),
    limit: u64,
) -> Result<Option<Vec<u8>>> {
    let mut message = Vec::new();
    reader.take(limit).read_until(b'\n', &mut message).await?;
    if message.is_empty() {
        return Ok(None);
    }
    if message.last() != Some(&b'\n') {
        bail!("missing or oversized runtime message");
    }
    Ok(Some(message))
}

/// Write one value as a newline-terminated JSON message.
///
/// # Errors
/// Fails when the value is longer than `limit` or the connection breaks.
pub async fn write_message(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &impl Serialize,
    limit: u64,
) -> Result<()> {
    let mut message = serde_json::to_vec(value)?;
    message.push(b'\n');
    if message.len() as u64 > limit {
        bail!("runtime message exceeds the frame limit");
    }
    stream.write_all(&message).await?;
    stream.flush().await?;
    Ok(())
}
