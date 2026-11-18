//! Agent adapters. Each adapter knows how to launch its CLI in PTY mode and
//! in structured (JSON stream) mode. Flags are checked against the installed
//! version before use.

mod agent_kind;
mod installed;

pub use agent_kind::{AgentKind, Launch};
pub use installed::{detect, detect_all, find_on_path, Installed, PromptDelivery, ResolvedCommand};
