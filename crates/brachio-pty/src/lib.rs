//! PTY sessions and the output batcher.
//!
//! The emulator state (`alacritty_terminal`) and replay on re-attach land in
//! M1.

mod exit_watch;
mod output_batcher;
mod pty_session;
mod session_manager;

pub use output_batcher::{BatchConfig, OutputBatcher};
pub use pty_session::{without_verbatim_prefix, PtySession, SpawnSpec};
pub use session_manager::{EventSink, PtyEvent, SessionId, SessionManager};
