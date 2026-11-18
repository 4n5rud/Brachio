//! Agent adapters. Each adapter knows how to launch its CLI in PTY mode and
//! in structured (JSON stream) mode. Flags are checked against the installed
//! version before use.

mod agent_kind;
mod codex_sessions;
mod installed;

pub use agent_kind::{AgentKind, Launch};
pub use codex_sessions::{latest_codex_session, latest_session_in};
pub use installed::{detect, detect_all, find_on_path, Installed, PromptDelivery, ResolvedCommand};
