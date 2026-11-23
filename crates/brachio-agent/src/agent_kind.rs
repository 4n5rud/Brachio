//! The supported agent CLIs and how to start them.

use serde::{Deserialize, Serialize};

use crate::catalog::{AgentSpec, ResumeSupport};

/// A supported agent CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// Anthropic's Claude Code (`claude`).
    ClaudeCode,
    /// OpenAI's Codex CLI (`codex`).
    Codex,
    /// xAI's Grok CLI (`grok`).
    Grok,
}

/// A program name and its argument vector. Never joined into a shell string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// Program name, looked up on `PATH`.
    pub program: &'static str,
    /// Arguments, one element per argument.
    pub args: Vec<String>,
}

impl AgentKind {
    /// The catalog entry that describes this agent.
    #[must_use]
    pub const fn spec(self) -> &'static AgentSpec {
        match self {
            Self::ClaudeCode => &crate::catalog::CLAUDE_CODE,
            Self::Codex => &crate::catalog::CODEX,
            Self::Grok => &crate::catalog::GROK,
        }
    }

    /// The agent with this stored id.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.spec().id == id)
    }

    /// Every supported agent, in display order.
    pub const ALL: [Self; 3] = [Self::ClaudeCode, Self::Codex, Self::Grok];

    /// The executable name on `PATH`.
    #[must_use]
    pub fn program(self) -> &'static str {
        self.spec().program
    }

    /// Interactive TUI, shown in a terminal panel.
    #[must_use]
    pub fn interactive(self) -> Launch {
        Launch {
            program: self.program(),
            args: Vec::new(),
        }
    }

    /// Interactive TUI that starts working on `prompt` right away.
    #[must_use]
    pub fn interactive_with_prompt(self, prompt: &str) -> Launch {
        let mut args = Vec::new();
        if prompt.starts_with('-') {
            args.push("--".to_owned());
        }
        args.push(prompt.to_owned());
        Launch {
            program: self.program(),
            args,
        }
    }

    /// Whether Brachio can choose the conversation id up front. Claude Code
    /// takes one; Codex picks its own, found later in its session files.
    #[must_use]
    pub fn accepts_session_id(self) -> bool {
        self.spec().chooses_session_id
    }

    /// Arguments that start a new conversation under `session_id`.
    /// Empty for agents that do not accept one.
    #[must_use]
    pub fn session_id_args(self, session_id: &str) -> Vec<String> {
        match self {
            Self::ClaudeCode => vec!["--session-id".to_owned(), session_id.to_owned()],
            Self::Codex | Self::Grok => Vec::new(),
        }
    }

    /// Interactive TUI that continues the conversation `session_id`. An agent without resume
    /// support starts a fresh conversation instead; check [`AgentSpec::resume`] first.
    #[must_use]
    pub fn resume(self, session_id: &str) -> Launch {
        let flag = match self.spec().resume {
            ResumeSupport::Flag(flag) | ResumeSupport::Subcommand(flag) => flag,
            ResumeSupport::None => return self.interactive(),
        };
        Launch {
            program: self.program(),
            args: vec![flag.to_owned(), session_id.to_owned()],
        }
    }

    /// Headless worker that emits one JSON event per line. Grok has no such mode in Brachio, so
    /// it gets its interactive launch.
    #[must_use]
    pub fn structured(self, prompt: &str) -> Launch {
        let args: Vec<&str> = match self {
            Self::ClaudeCode => vec!["-p", prompt, "--output-format", "stream-json", "--verbose"],
            Self::Codex => vec!["exec", "--json", prompt],
            Self::Grok => return self.interactive_with_prompt(prompt),
        };
        Launch {
            program: self.program(),
            args: args.into_iter().map(str::to_owned).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_dash_remains_part_of_the_positional_prompt() {
        for kind in AgentKind::ALL {
            assert_eq!(
                kind.interactive_with_prompt("- summarize this").args,
                ["--", "- summarize this"]
            );
        }
    }

    #[test]
    fn prompt_is_one_argument_not_shell_text() {
        let launch = AgentKind::Codex.structured("fix it; rm -rf /");
        assert_eq!(launch.args, vec!["exec", "--json", "fix it; rm -rf /"]);
    }

    #[test]
    fn interactive_prompt_is_the_only_argument() {
        let launch = AgentKind::ClaudeCode.interactive_with_prompt("add a test");
        assert_eq!(launch.args, vec!["add a test"]);
    }

    #[test]
    fn claude_structured_uses_stream_json() {
        let launch = AgentKind::ClaudeCode.structured("hi");
        assert_eq!(launch.program, "claude");
        let has_flag = launch
            .args
            .windows(2)
            .any(|pair| pair == ["--output-format", "stream-json"]);
        assert!(
            has_flag,
            "stream-json output is required: {:?}",
            launch.args
        );
    }

    #[test]
    fn resume_names_the_conversation() {
        assert_eq!(
            AgentKind::ClaudeCode.resume("abc").args,
            vec!["--resume", "abc"]
        );
        assert_eq!(AgentKind::Codex.resume("abc").args, vec!["resume", "abc"]);
    }

    #[test]
    fn every_agent_has_a_unique_id_that_finds_it_again() {
        for kind in AgentKind::ALL {
            assert_eq!(AgentKind::from_id(kind.spec().id), Some(kind));
        }
        let ids: Vec<_> = AgentKind::ALL.iter().map(|kind| kind.spec().id).collect();
        assert_eq!(ids, ["claude_code", "codex", "grok"]);
        assert_eq!(AgentKind::from_id("gemini"), None);
    }

    #[test]
    fn the_serialized_name_is_the_catalog_id() {
        for kind in AgentKind::ALL {
            assert_eq!(
                serde_json::to_string(&kind).expect("json"),
                format!("\"{}\"", kind.spec().id)
            );
        }
    }

    #[test]
    fn only_claude_relays_approvals_into_the_app() {
        let relaying: Vec<_> = AgentKind::ALL
            .into_iter()
            .filter(|kind| kind.spec().approval_relay)
            .collect();
        assert_eq!(relaying, [AgentKind::ClaudeCode]);
    }

    #[test]
    fn grok_reports_by_osc_7501_and_cannot_resume() {
        let grok = AgentKind::Grok.spec();
        assert_eq!(grok.status, crate::catalog::StatusChannel::Osc7501);
        assert_eq!(grok.resume, crate::catalog::ResumeSupport::None);
        assert_eq!(AgentKind::Grok.resume("abc"), AgentKind::Grok.interactive());
        assert!(!AgentKind::Grok.accepts_session_id());
    }
}
