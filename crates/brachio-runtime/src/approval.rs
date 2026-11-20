//! Answering an agent's permission question from the app instead of from the terminal.
//!
//! The agent runs a hook command (`brachio hook permission-request`) when it is about to ask
//! the user for permission. The hook forwards the question to the app over a local socket and
//! turns the user's click into the answer format the agent understands. Every failure ends with
//! no answer at all, so the agent falls back to its own question in the terminal. Nothing here
//! can approve on its own.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio::io::BufReader;

use crate::transport;
use crate::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};

/// Names the socket the app listens on for permission questions.
pub const ENDPOINT_VARIABLE: &str = "BRACHIO_HOOK_ENDPOINT";
/// Names the terminal the agent runs in, such as `<task id>:agent`.
pub const KEY_VARIABLE: &str = "BRACHIO_TERMINAL_KEY";
/// How long the hook waits for the user, in seconds.
pub const WAIT_VARIABLE: &str = "BRACHIO_APPROVAL_WAIT_SECONDS";
/// The wait when [`WAIT_VARIABLE`] is not set.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(300);

/// Larger requests go to the agent's own permission prompt.
const MAX_REVIEW_BYTES: usize = 2_000;

fn is_unsafe_display_char(character: char) -> bool {
    (character.is_control() && character != '\n')
        || matches!(
            character,
            '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
        )
}

/// One permission question from an agent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ApprovalRequest {
    /// The terminal the agent runs in.
    pub key: String,
    /// The agent's conversation.
    pub session_id: String,
    /// The tool the agent wants to use, such as `Bash`.
    pub tool_name: String,
    /// The complete JSON input. The new wire field makes legacy previews fall back to the terminal.
    pub review_input: String,
    /// Identifies this use of the tool.
    pub tool_use_id: String,
}

impl ApprovalRequest {
    /// The complete tool name and input as displayable JSON, or `None` to defer to the terminal.
    #[must_use]
    pub fn review(&self) -> Option<String> {
        let name = serde_json::to_string(&self.tool_name).ok()?;
        let input = &self.review_input;
        // Refuse control characters (except newline) and bidirectional or invisible formatting
        // characters, which could reorder or hide text in the approval view; other Unicode is shown.
        if name.len().saturating_add(input.len()) >= MAX_REVIEW_BYTES
            || self.tool_name.is_empty()
            || !input.starts_with('{')
            || name
                .chars()
                .chain(input.chars())
                .any(is_unsafe_display_char)
        {
            return None;
        }
        serde_json::from_str::<&RawValue>(input).ok()?;
        Some(format!("{name}\n{input}"))
    }
}

/// The user's answer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "behavior", rename_all = "snake_case")]
pub enum Decision {
    /// Run the tool.
    Allow,
    /// Do not run the tool, and tell the agent why.
    Deny {
        /// Shown to the agent.
        message: String,
    },
    /// No decision: let the agent ask in the terminal.
    Ask,
}

/// What an agent sends a permission hook on standard input.
#[derive(Deserialize)]
struct HookInput {
    session_id: String,
    tool_name: String,
    tool_input: Box<RawValue>,
    #[serde(default)]
    tool_use_id: String,
}

/// Reads the hook's standard input. `None` when it is not a permission question.
#[must_use]
pub fn request_from_hook_input(input: &str, key: &str) -> Option<ApprovalRequest> {
    let parsed: HookInput = serde_json::from_str(input).ok()?;
    let request = ApprovalRequest {
        key: key.to_owned(),
        session_id: parsed.session_id,
        tool_name: parsed.tool_name,
        review_input: parsed.tool_input.get().to_owned(),
        tool_use_id: parsed.tool_use_id,
    };
    request.review()?;
    Some(request)
}

/// The text the hook prints for an agent. `None` prints nothing, which leaves the question to
/// the agent's own terminal prompt.
#[must_use]
pub fn hook_output(decision: &Decision) -> Option<String> {
    let decision = match decision {
        Decision::Allow => serde_json::json!({ "behavior": "allow" }),
        Decision::Deny { message } => {
            serde_json::json!({ "behavior": "deny", "message": message, "reason": message })
        }
        Decision::Ask => return None,
    };
    Some(
        serde_json::json!({
            "continue": true,
            "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision }
        })
        .to_string(),
    )
}

/// Asks the app and waits for the user. `None` means there is no answer, for whatever reason: the
/// app is not running, it closed the connection, or the wait ran out.
pub async fn ask(endpoint: &str, request: &ApprovalRequest, wait: Duration) -> Option<Decision> {
    let exchange = async {
        let mut stream = transport::connect(endpoint).await.ok()?;
        write_message(&mut stream, request, MAX_MESSAGE)
            .await
            .ok()?;
        let mut reader = BufReader::new(stream);
        let message = read_message(&mut reader, MAX_RESPONSE).await.ok()??;
        serde_json::from_slice::<Decision>(&message).ok()
    };
    tokio::time::timeout(wait, exchange).await.ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::{hook_output, request_from_hook_input, ApprovalRequest, Decision};

    #[test]
    fn a_permission_question_becomes_a_request_with_the_terminal_key() {
        let input = r#"{"session_id":"s1","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"pnpm add zod"},"tool_use_id":"t1"}"#;
        let request = request_from_hook_input(input, "task1:agent").expect("request");
        assert_eq!(request.key, "task1:agent");
        assert_eq!(request.tool_name, "Bash");
        assert_eq!(
            request.review().as_deref(),
            Some("\"Bash\"\n{\"command\":\"pnpm add zod\"}")
        );
    }

    #[test]
    fn input_that_is_not_a_permission_question_is_ignored() {
        assert!(request_from_hook_input("not json", "k").is_none());
        assert!(request_from_hook_input(r#"{"tool_name":"Bash"}"#, "k").is_none());
    }

    #[test]
    fn long_nested_input_goes_to_the_terminal() {
        let long = "x".repeat(5_000);
        let input = format!(
            r#"{{"session_id":"s","tool_name":"Write","tool_input":{{"files":[{{"content":"{long}"}}]}}}}"#
        );
        assert!(request_from_hook_input(&input, "k").is_none());
    }

    #[test]
    fn review_keeps_all_fields_and_the_original_json_text() {
        let input = r#"{"session_id":"s","tool_name":"Edit","tool_input":{"file_path":"src/a.ts","old_string":"x","new_string":"y\nz","offset":9007199254740993}}"#;
        let request = request_from_hook_input(input, "k").expect("request");
        assert_eq!(
            request.review().expect("review"),
            concat!(
                "\"Edit\"\n",
                r#"{"file_path":"src/a.ts","old_string":"x","new_string":"y\nz","offset":9007199254740993}"#
            )
        );
    }

    #[test]
    fn missing_input_and_unsafe_display_text_go_to_the_terminal() {
        assert!(request_from_hook_input(r#"{"session_id":"s","tool_name":"Edit"}"#, "k").is_none());
        for unsafe_char in ['\u{202e}', '\u{200b}', '\u{0085}'] {
            let input = format!(
                r#"{{"session_id":"s","tool_name":"Edit","tool_input":{{"content":"abc{unsafe_char}def"}}}}"#,
            );
            assert!(request_from_hook_input(&input, "k").is_none());
        }
    }

    #[test]
    fn korean_text_is_shown_unchanged() {
        let input =
            r#"{"session_id":"s","tool_name":"Bash","tool_input":{"command":"echo 안녕하세요"}}"#;
        let request = request_from_hook_input(input, "k").expect("request");
        assert_eq!(
            request.review().expect("review"),
            "\"Bash\"\n{\"command\":\"echo 안녕하세요\"}"
        );
    }

    #[test]
    fn legacy_hook_messages_cannot_open_an_app_approval() {
        let old =
            r#"{"key":"k","session_id":"s","tool_name":"Edit","tool_input":{},"tool_use_id":"t"}"#;
        assert!(serde_json::from_str::<ApprovalRequest>(old).is_err());
    }

    #[test]
    fn allow_and_deny_print_the_agents_answer_format_and_ask_prints_nothing() {
        let allow: serde_json::Value =
            serde_json::from_str(&hook_output(&Decision::Allow).expect("output")).expect("json");
        assert_eq!(
            allow["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(allow["hookSpecificOutput"]["decision"]["behavior"], "allow");

        let deny: serde_json::Value = serde_json::from_str(
            &hook_output(&Decision::Deny {
                message: "Denied in Brachio".into(),
            })
            .expect("output"),
        )
        .expect("json");
        assert_eq!(deny["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(
            deny["hookSpecificOutput"]["decision"]["message"],
            "Denied in Brachio"
        );

        assert!(hook_output(&Decision::Ask).is_none());
    }
}
