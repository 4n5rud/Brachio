//! Agent adapters. Each adapter knows how to launch its CLI in PTY mode and
//! in structured (JSON stream) mode. Flags are checked against the installed
//! version before use.

mod agent_kind;
mod catalog;
mod codex_sessions;
mod installed;
mod permissions;
mod profile;
mod session_tokens;
mod usage_scan;

pub use agent_kind::{AgentKind, Launch};
pub use catalog::{AgentSpec, ResumeSupport, StatusChannel};
pub use codex_sessions::{
    codex_brachio_instructions, codex_conversation_started_with, codex_conversations,
    codex_conversations_in, codex_developer_instructions, codex_rollout_path,
    codex_user_instructions, codex_worktree_conversations, is_tab_marker, latest_codex_session,
    latest_session_in, rollout_in, CodexConversation, WorktreeConversations, MAX_INSTRUCTIONS,
    MAX_WINDOWS_INSTRUCTIONS,
};
pub use installed::{
    detect, detect_all, find_on_path, is_safe_for_cmd, locate, Installed, PromptDelivery,
    ResolvedCommand,
};
pub use permissions::{read_policy, PermissionPolicy, PolicyDecision, READ_ONLY_TOOLS};
pub use profile::{
    find_profile, parse_profiles, read_profiles, AgentProfile, ProfileEntry, ProfileError,
    BYPASS_WORDS,
};
pub use session_tokens::{
    claude_conversation_recorded, claude_log_in, claude_log_path, encode_claude_cwd, LogFormat,
    TokenUsage, UsageTracker,
};
pub use usage_scan::{
    claude_logs_in, codex_logs_in, providers, Bucket, ClaudeProvider, CodexProvider, FileState,
    LogFile, Scanned, UsageProvider, MAX_LOG_FILES,
};
