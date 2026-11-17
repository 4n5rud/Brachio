//! Agent adapters. Each adapter knows how to launch its CLI in PTY mode and
//! in structured (JSON stream) mode. Flags are checked against the installed
//! version before use.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    ClaudeCode,
    Codex,
}

/// A program and its argument vector. Never joined into a shell string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: &'static str,
    pub args: Vec<String>,
}

impl AgentKind {
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
            args: vec![],
        }
    }

    /// Headless worker that emits one JSON event per line.
    #[must_use]
    pub fn structured(self, prompt: &str) -> Launch {
        let args = match self {
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
        let l = AgentKind::Codex.structured("fix it; rm -rf /");
        assert_eq!(l.args, vec!["exec", "--json", "fix it; rm -rf /"]);
    }

    #[test]
    fn claude_structured_uses_stream_json() {
        let l = AgentKind::ClaudeCode.structured("hi");
        assert_eq!(l.program, "claude");
        assert!(l
            .args
            .windows(2)
            .any(|w| w == ["--output-format", "stream-json"]));
    }
}
