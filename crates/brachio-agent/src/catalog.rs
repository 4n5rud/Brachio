//! What Brachio knows about each supported agent, in one table.

/// How Brachio learns what an agent is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusChannel {
    /// Session-scoped hooks the app installs for the agent.
    Hooks,
    /// The agent reports itself with OSC 7501 on its terminal stream.
    Osc7501,
    /// Nothing; the terminal is all there is.
    None,
}

/// How a stored conversation is continued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeSupport {
    /// `<program> <flag> <id>`.
    Flag(&'static str),
    /// `<program> <subcommand> <id>`.
    Subcommand(&'static str),
    /// Every start is a new conversation.
    None,
}

/// One supported agent.
#[derive(Debug, PartialEq, Eq)]
pub struct AgentSpec {
    /// Stored in the database and sent to the window.
    pub id: &'static str,
    /// Shown to the user.
    pub name: &'static str,
    /// Executable looked up on `PATH`.
    pub program: &'static str,
    /// How a conversation is continued.
    pub resume: ResumeSupport,
    /// Whether Brachio can choose the conversation id up front.
    pub chooses_session_id: bool,
    /// Where the agent's status comes from.
    pub status: StatusChannel,
    /// Whether the app answers the agent's permission questions. Without it the user approves
    /// in the agent's own terminal.
    pub approval_relay: bool,
    /// The icon the window draws for it.
    pub brand_icon: &'static str,
}

pub const CLAUDE_CODE: AgentSpec = AgentSpec {
    id: "claude_code",
    name: "Claude Code",
    program: "claude",
    resume: ResumeSupport::Flag("--resume"),
    chooses_session_id: true,
    status: StatusChannel::Hooks,
    approval_relay: true,
    brand_icon: "claude",
};

pub const CODEX: AgentSpec = AgentSpec {
    id: "codex",
    name: "Codex",
    program: "codex",
    resume: ResumeSupport::Subcommand("resume"),
    chooses_session_id: false,
    status: StatusChannel::Osc7501,
    approval_relay: false,
    brand_icon: "codex",
};

pub const GROK: AgentSpec = AgentSpec {
    id: "grok",
    name: "Grok",
    program: "grok",
    resume: ResumeSupport::None,
    chooses_session_id: false,
    status: StatusChannel::Osc7501,
    approval_relay: false,
    brand_icon: "grok",
};
