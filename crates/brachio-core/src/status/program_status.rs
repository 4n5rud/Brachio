//! The OSC 7501 Program Status protocol: an agent reports its own state
//! through its terminal output.
//!
//! Wire format: `ESC ] 7501 ; key=value:key=value ESC \`. A program first
//! sends the query `?` and only reports once the terminal answers it.
//! Spec: <https://www.superlogical.com/rex/docs/build/program-status>

use base64::Engine;

use super::agent_state::{AgentState, BlockReason};

/// The answer to a query. It lists every state and kind Brachio reads.
pub const QUERY_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

/// Longest decoded `msg` the protocol allows, in bytes.
const MAX_MESSAGE_BYTES: usize = 2048;

/// One parsed OSC 7501 body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramStatusMessage {
    /// The program asks whether the terminal speaks the protocol.
    Query,
    /// The program reports the state of its main task.
    Report(ProgramStatus),
}

/// A program's report about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStatus {
    /// The state, mapped onto Brachio's states. `Unknown` clears the record.
    pub state: AgentState,
    /// Optional one-line detail, such as the command waiting for approval.
    pub message: Option<String>,
}

impl ProgramStatusMessage {
    /// Parses the text between `ESC ] 7501 ;` and the terminator.
    ///
    /// Returns `None` for bodies Brachio ignores: no valid state, a report
    /// about a sub-task (`id=`), or a malformed message.
    #[must_use]
    pub fn parse(body: &str) -> Option<Self> {
        if body.trim() == "?" {
            return Some(Self::Query);
        }
        let mut state = None;
        let mut kind = None;
        let mut message = None;
        for pair in body.split(':') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            match key.trim() {
                "state" => state = Some(value.trim()),
                "kind" => kind = Some(value.trim()),
                "msg" => message = Some(decode_message(value.trim())?),
                // Sub-tasks have their own records. Only the root one is the agent's state.
                "id" => return None,
                _ => {}
            }
        }
        let state = match state? {
            "working" => AgentState::Working,
            // `done` means the turn finished and the agent waits for input.
            "idle" | "done" => AgentState::Idle,
            "error" => AgentState::Error,
            "blocked" => AgentState::Blocked {
                reason: block_reason(kind),
            },
            "clear" => AgentState::Unknown,
            _ => return None,
        };
        Some(Self::Report(ProgramStatus {
            state,
            message: message.filter(|text| !text.is_empty()),
        }))
    }
}

fn block_reason(kind: Option<&str>) -> BlockReason {
    match kind {
        Some("permission") => BlockReason::Permission,
        Some("question") => BlockReason::Question,
        Some("auth") => BlockReason::Auth,
        _ => BlockReason::Other,
    }
}

/// Decodes a base64 `msg`. Control characters void it, since it is shown as one line.
fn decode_message(encoded: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    if text.chars().any(char::is_control) {
        return None;
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(body: &str) -> ProgramStatus {
        match ProgramStatusMessage::parse(body) {
            Some(ProgramStatusMessage::Report(status)) => status,
            other => panic!("expected a report for {body:?}, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_query() {
        assert_eq!(
            ProgramStatusMessage::parse(" ? "),
            Some(ProgramStatusMessage::Query)
        );
    }

    #[test]
    fn maps_states() {
        assert_eq!(report("state=working").state, AgentState::Working);
        assert_eq!(report("state=done").state, AgentState::Idle);
        assert_eq!(report("state=error").state, AgentState::Error);
        assert_eq!(report("state=clear").state, AgentState::Unknown);
    }

    #[test]
    fn reads_the_block_reason_and_message() {
        // "npm test" in base64.
        let status = report("state=blocked:kind=permission:msg=bnBtIHRlc3Q=");
        assert_eq!(
            status.state,
            AgentState::Blocked {
                reason: BlockReason::Permission
            }
        );
        assert_eq!(status.message.as_deref(), Some("npm test"));
        assert_eq!(
            report("state=blocked").state,
            AgentState::Blocked {
                reason: BlockReason::Other
            }
        );
    }

    #[test]
    fn ignores_sub_tasks_and_bad_bodies() {
        assert_eq!(ProgramStatusMessage::parse("id=build:state=working"), None);
        assert_eq!(ProgramStatusMessage::parse("state=sleeping"), None);
        assert_eq!(ProgramStatusMessage::parse("kind=permission"), None);
        assert_eq!(ProgramStatusMessage::parse("state=idle:msg=!!!"), None);
    }
}
