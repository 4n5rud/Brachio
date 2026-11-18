//! The supported agent CLIs and how to start them.

use serde::{Deserialize, Serialize};

/// A supported agent CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// Anthropic's Claude Code (`claude`).
    ClaudeCode,
    /// OpenAI's Codex CLI (`codex`).
    Codex,
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
    /// Every supported agent, in display order.
    pub const ALL: [Self; 2] = [Self::ClaudeCode, Self::Codex];

    /// The executable name on `PATH`.
    #[must_use]
    pub fn program(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Codex => "codex",
        }
    }

    /// Interactive TUI, shown in a terminal panel.
    #[must_use]
    pub fn interactive(self) -> Launch {
        Launch {
            program: self.program(),
            args: Vec::new(),
        }
    }

    /// Headless worker that emits one JSON event per line.
    #[must_use]
    pub fn structured(self, prompt: &str) -> Launch {
        let args: Vec<&str> = match self {
            Self::ClaudeCode => vec!["-p", prompt, "--output-format", "stream-json", "--verbose"],
            Self::Codex => vec!["exec", "--json", prompt],
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
    fn prompt_is_one_argument_not_shell_text() {
        let launch = AgentKind::Codex.structured("fix it; rm -rf /");
        assert_eq!(launch.args, vec!["exec", "--json", "fix it; rm -rf /"]);
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
}
