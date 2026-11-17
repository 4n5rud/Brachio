//! PTY sessions and the output batcher.
//!
//! The emulator state (`alacritty_terminal`) and replay on re-attach land in
//! M1.

mod batch;
mod session;

pub use batch::{BatchConfig, OutputBatcher};
pub use session::{PtySession, SpawnSpec};
