//! Agent adapters. Each adapter knows how to launch its CLI in PTY mode and
//! in structured (JSON stream) mode. Flags are checked against the installed
//! version before use.

mod agent_kind;
mod catalog;
mod codex_sessions;
mod installed;
mod session_tokens;
mod usage_scan;

pub use agent_kind::{AgentKind, Launch};
pub use catalog::{AgentSpec, ResumeSupport, StatusChannel};
pub use codex_sessions::{
    codex_conversations, codex_rollout_path, latest_codex_session, latest_session_in, rollout_in,
    CodexConversation,
};
pub use installed::{
    detect, detect_all, find_on_path, is_safe_for_cmd, locate, Installed, PromptDelivery,
    ResolvedCommand,
};
pub use session_tokens::{
    claude_log_in, claude_log_path, encode_claude_cwd, LogFormat, TokenUsage, UsageTracker,
};
pub use usage_scan::{
    claude_logs_in, codex_logs_in, providers, Bucket, ClaudeProvider, CodexProvider, FileState,
    LogFile, Scanned, UsageProvider, MAX_LOG_FILES,
};
