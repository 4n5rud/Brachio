//! `brachio hook mail`: tells a Claude Code agent, at no cost when there is nothing to tell, that
//! mail is waiting.
//!
//! Claude Code runs this after each tool call and when the user sends a prompt. It prints one
//! line of context only when the number of unread messages has grown since the last time it
//! spoke, so an agent with no mail pays no tokens, and one with mail is told once per new message.
//! Every failure prints nothing and the agent carries on.

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use brachio_runtime::approval::{ENDPOINT_VARIABLE, KEY_VARIABLE};
use brachio_runtime::mailbox::{self, MailRequest, MailResponse};

/// Overrides where the last reported count is kept, for tests.
const STATE_VARIABLE: &str = "BRACHIO_MAIL_STATE_DIR";

pub fn run() {
    let (Ok(endpoint), Ok(key)) = (
        std::env::var(ENDPOINT_VARIABLE),
        std::env::var(KEY_VARIABLE),
    ) else {
        return;
    };
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return;
    }
    let Some(event) = event_name(&input) else {
        return;
    };
    let Some(unread) = unread_count(&endpoint, &key) else {
        return;
    };
    let state = state_file(&key);
    let reported = fs::read_to_string(&state)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
        .unwrap_or(0);
    if unread != reported {
        let _ = fs::write(&state, unread.to_string());
    }
    if let Some(output) = announcement(&event, unread, reported) {
        println!("{output}");
    }
}

fn event_name(input: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
    let event = parsed.get("hook_event_name")?.as_str()?.to_owned();
    matches!(event.as_str(), "PostToolUse" | "UserPromptSubmit").then_some(event)
}

fn unread_count(endpoint: &str, key: &str) -> Option<u32> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    let request = MailRequest::Check {
        key: key.to_owned(),
        limit: Some(1),
        peek: true,
    };
    match runtime.block_on(mailbox::call(endpoint, &request)).ok()? {
        MailResponse::Messages { unread_left, .. } => Some(unread_left),
        _ => None,
    }
}

/// The hook output, or `None` when there is nothing new to say.
fn announcement(event: &str, unread: u32, reported: u32) -> Option<String> {
    if unread == 0 || unread <= reported {
        return None;
    }
    let plural = if unread == 1 { "message" } else { "messages" };
    Some(
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": event,
                "additionalContext":
                    format!("Brachio: {unread} unread {plural}. Run `brachio check` when convenient.")
            }
        })
        .to_string(),
    )
}

fn state_file(key: &str) -> PathBuf {
    let folder = std::env::var_os(STATE_VARIABLE).map_or_else(std::env::temp_dir, PathBuf::from);
    let safe: String = key
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    folder.join(format!("brachio-mail-{safe}"))
}

#[cfg(test)]
mod tests {
    use super::announcement;

    #[test]
    fn it_speaks_only_when_the_unread_count_has_grown() {
        assert!(announcement("PostToolUse", 0, 0).is_none());
        assert!(announcement("PostToolUse", 2, 2).is_none(), "already told");
        assert!(
            announcement("PostToolUse", 1, 2).is_none(),
            "some were read"
        );
        let first = announcement("UserPromptSubmit", 1, 0).expect("new mail");
        let parsed: serde_json::Value = serde_json::from_str(&first).expect("json");
        assert_eq!(
            parsed["hookSpecificOutput"]["hookEventName"],
            "UserPromptSubmit"
        );
        assert!(parsed["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .is_some_and(|text| text.contains("1 unread message.")));
        assert!(announcement("PostToolUse", 3, 1).is_some(), "more arrived");
    }
}
