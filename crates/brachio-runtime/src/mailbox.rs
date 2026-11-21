//! Messages between agent sessions, over the same local socket the permission hook uses.
//!
//! An agent runs `brachio send`, `brachio check` or `brachio peers`; each is one request to the
//! app and one answer. The app keeps the messages. Nothing here puts a message into another
//! agent's input: the receiver asks for its mail when it wants it.

use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::io::BufReader;

use crate::transport;
use crate::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};

/// How long a command waits for the app to answer.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// How long `brachio task create` waits: the user is asked first and may take a while.
pub const CREATE_TASK_TIMEOUT: Duration = Duration::from_secs(300);
/// `brachio task wait` waits this long unless told otherwise.
pub const DEFAULT_WAIT_SECS: u64 = 600;
/// The longest `brachio task wait` may be asked to wait.
pub const MAX_WAIT_SECS: u64 = 3600;
/// How long the app waits for the client to acknowledge the messages of a `check` before it
/// leaves them unread.
pub const ACK_TIMEOUT: Duration = Duration::from_secs(5);
/// Messages returned by one `check` unless asked for fewer.
pub const DEFAULT_BATCH: usize = 10;
/// The most messages one request can return.
pub const MAX_BATCH: usize = 20;

/// A message to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMail {
    /// Sender's terminal key.
    pub from: String,
    /// Receiver: a terminal key, task id or title. Empty for replies inferred from the parent.
    pub to: String,
    /// Message type, such as `status` or `question`.
    pub kind: String,
    /// `low`, `normal` or `high`.
    #[serde(default)]
    pub priority: Option<String>,
    /// One line.
    pub subject: String,
    /// Details.
    #[serde(default)]
    pub body: String,
    /// Structured references such as file paths, as JSON text.
    #[serde(default)]
    pub payload: Option<String>,
    /// The message this answers.
    #[serde(default)]
    pub reply_to: Option<String>,
    /// Sending the same id again does not send a second message.
    #[serde(default)]
    pub request_id: Option<String>,
}

/// What an agent can ask the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MailRequest {
    /// Send a message.
    Send(SendMail),
    /// Take the oldest unread messages, or only look at them with `peek`.
    Check {
        /// The asking terminal.
        key: String,
        /// At most this many; [`DEFAULT_BATCH`] when absent.
        #[serde(default)]
        limit: Option<usize>,
        /// Leave the messages unread.
        #[serde(default)]
        peek: bool,
    },
    /// Sent on the connection of a consuming `check` once its messages were received and shown.
    /// Only then does the app mark exactly these messages read; a client that never acknowledges
    /// (a closed connection, a timeout, or an older `brachio` that does not know this request)
    /// leaves them unread and they are shown again by the next `check`.
    Ack {
        /// The ids of the messages the `check` answer carried.
        ids: Vec<String>,
    },
    /// The latest messages to or from the terminal, read or not.
    Inbox {
        /// The asking terminal.
        key: String,
        /// At most this many; [`DEFAULT_BATCH`] when absent.
        #[serde(default)]
        limit: Option<usize>,
    },
    /// The open tasks a message can be sent to.
    Peers {
        /// The asking terminal.
        key: String,
    },
    /// Ask the user to create a child task of the asking terminal's task.
    CreateTask {
        /// The asking terminal.
        key: String,
        /// Title of the new task.
        title: String,
        /// The prompt the new task's agent starts with.
        prompt: String,
        /// `claude` or `codex`; `claude` when absent.
        #[serde(default)]
        agent: Option<String>,
        /// Commit-ish to branch from; the asking task's branch when absent.
        #[serde(default)]
        base: Option<String>,
        /// Terminal keys or ids of sibling child tasks that must be completed first.
        #[serde(default)]
        after: Vec<String>,
    },
    /// A child task hands its committed work over for the user's review.
    Handoff {
        /// The asking terminal.
        key: String,
        /// What the agent says it did; untrusted text.
        summary: String,
        /// Tests the agent says it ran; untrusted claims.
        #[serde(default)]
        tests: Vec<String>,
    },
    /// The child tasks of the asking lead.
    ListTasks {
        /// The asking terminal.
        key: String,
    },
    /// Hold the answer until the lead's open children have reported done, or time runs out.
    WaitTasks {
        /// The asking terminal.
        key: String,
        /// Wait for every open child; otherwise for the first one.
        #[serde(default)]
        all: bool,
        /// Longest wait, capped at [`MAX_WAIT_SECS`].
        #[serde(default)]
        timeout_secs: u64,
    },
}

/// A message as the receiver sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailMessage {
    /// Identifier, for `--reply-to`.
    pub id: String,
    /// The conversation it belongs to.
    pub thread_id: String,
    /// Sender's terminal key.
    pub from: String,
    /// Receiver's terminal key.
    #[serde(default)]
    pub to: String,
    /// Sender's task title, when the task is known.
    #[serde(default)]
    pub from_title: Option<String>,
    /// Message type.
    pub kind: String,
    /// `low`, `normal` or `high`.
    pub priority: String,
    /// One line.
    pub subject: String,
    /// Details.
    pub body: String,
    /// References, as JSON text.
    #[serde(default)]
    pub payload: Option<String>,
    /// Sent at, in milliseconds since the Unix epoch.
    pub created_at_ms: i64,
    /// Whether it had been read before this request.
    pub read: bool,
}

/// A task a message can go to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    /// Terminal key to put after `--to`.
    pub key: String,
    /// The task's title.
    pub title: String,
    /// The task's branch.
    pub branch: String,
}

/// A child task as `brachio task list` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildTaskInfo {
    /// The child agent's terminal key.
    pub key: String,
    /// The child's title.
    pub title: String,
    /// The child is archived.
    #[serde(default)]
    pub archived: bool,
    /// The child's agent sent a `worker_done` message to the lead.
    #[serde(default)]
    pub reported: bool,
    /// Messages from the child the lead has not read.
    #[serde(default)]
    pub unread: u32,
}

/// A child that reported done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportedChild {
    /// The child agent's terminal key.
    pub key: String,
    /// Subject of its `worker_done` message.
    pub subject: String,
}

/// The app's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum MailResponse {
    /// A message was stored.
    Sent {
        /// Its identifier.
        id: String,
        /// Its conversation.
        thread_id: String,
        /// The terminal it went to.
        to: String,
        /// The request id had been used before, so nothing new was stored.
        duplicate: bool,
    },
    /// Messages for `check` and `inbox`.
    Messages {
        /// Oldest first for `check`, newest first for `inbox`.
        messages: Vec<MailMessage>,
        /// Unread messages left after this request.
        unread_left: u32,
    },
    /// Tasks for `peers`.
    Peers {
        /// The open tasks.
        peers: Vec<Peer>,
    },
    /// The user allowed a new task and it exists.
    TaskCreated {
        /// Terminal key of the new task's agent.
        key: String,
        /// The new task's title.
        title: String,
        /// The new task's branch.
        branch: String,
    },
    /// The handoff was stored and the task now waits for the user's review.
    HandedOff {
        /// The commit the handoff names.
        commit_sha: String,
        /// How many files differ from the base.
        changed_paths: usize,
    },
    /// Children for `task list`, oldest first.
    TaskList {
        /// The lead's child tasks.
        #[serde(default)]
        children: Vec<ChildTaskInfo>,
    },
    /// Open children that reported, for `task wait`.
    TasksReported {
        /// Open children that have reported.
        #[serde(default)]
        reported: Vec<ReportedChild>,
        /// Keys of open children that have not.
        #[serde(default)]
        pending: Vec<String>,
        /// The wait ended because time ran out.
        #[serde(default)]
        timed_out: bool,
    },
    /// The request was refused, and why.
    Error {
        /// What the agent should be told.
        message: String,
    },
}

/// True when a message on the socket is a mailbox request rather than a permission question.
#[must_use]
pub fn is_mail(message: &serde_json::Value) -> bool {
    message.get("op").is_some()
}

/// Sends one request and waits for the answer.
///
/// # Errors
/// Fails when the app is not running, closes the connection, or does not answer in time.
pub async fn call(endpoint: &str, request: &MailRequest) -> Result<MailResponse> {
    call_with_timeout(endpoint, request, CALL_TIMEOUT).await
}

/// Sends a consuming `check`, runs `show` on the answer, then acknowledges the messages so the app
/// marks them read. `show` runs before the acknowledgement, so a message that was not shown stays
/// unread. An answer without messages needs no acknowledgement.
///
/// # Errors
/// Fails like [`call`], or with the error `show` returns; nothing is acknowledged then.
pub async fn call_with_ack(
    endpoint: &str,
    request: &MailRequest,
    timeout: Duration,
    show: impl FnOnce(&MailResponse) -> Result<()>,
) -> Result<MailResponse> {
    let exchange = async {
        let mut stream = transport::connect(endpoint)
            .await
            .map_err(|_| anyhow!("Brachio is not running or does not accept messages."))?;
        write_message(&mut stream, request, MAX_MESSAGE).await?;
        let mut reader = BufReader::new(stream);
        let message = read_message(&mut reader, MAX_RESPONSE)
            .await?
            .ok_or_else(|| anyhow!("Brachio closed the connection without answering."))?;
        let response = serde_json::from_slice::<MailResponse>(&message)?;
        show(&response)?;
        if let MailResponse::Messages { messages, .. } = &response {
            if !messages.is_empty() {
                let ids = messages.iter().map(|message| message.id.clone()).collect();
                // The messages were shown; a failed acknowledgement only leaves them unread.
                if write_message(reader.get_mut(), &MailRequest::Ack { ids }, MAX_MESSAGE)
                    .await
                    .is_ok()
                {
                    // The app closes once it has handled the acknowledgement.
                    let _ = read_message(&mut reader, MAX_RESPONSE).await;
                }
            }
        }
        Ok::<_, anyhow::Error>(response)
    };
    tokio::time::timeout(timeout, exchange)
        .await
        .map_err(|_| anyhow!("Brachio did not answer in time."))?
}

/// Like [`call`], with a different time to wait for the answer.
///
/// # Errors
/// Fails when the app is not running, closes the connection, or does not answer in time.
pub async fn call_with_timeout(
    endpoint: &str,
    request: &MailRequest,
    timeout: Duration,
) -> Result<MailResponse> {
    let exchange = async {
        let mut stream = transport::connect(endpoint)
            .await
            .map_err(|_| anyhow!("Brachio is not running or does not accept messages."))?;
        write_message(&mut stream, request, MAX_MESSAGE).await?;
        let mut reader = BufReader::new(stream);
        let message = read_message(&mut reader, MAX_RESPONSE)
            .await?
            .ok_or_else(|| anyhow!("Brachio closed the connection without answering."))?;
        Ok::<_, anyhow::Error>(serde_json::from_slice::<MailResponse>(&message)?)
    };
    tokio::time::timeout(timeout, exchange)
        .await
        .map_err(|_| anyhow!("Brachio did not answer in time."))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mail_request_is_told_apart_from_a_permission_question() {
        let mail = serde_json::to_value(MailRequest::Peers {
            key: "t:agent".into(),
        })
        .expect("json");
        assert!(is_mail(&mail));
        let question = serde_json::json!({ "key": "t:agent", "tool_name": "Bash" });
        assert!(!is_mail(&question));
    }

    #[test]
    fn requests_and_answers_survive_a_round_trip() {
        let request = MailRequest::Send(SendMail {
            from: "a:agent".into(),
            to: "b".into(),
            kind: "question".into(),
            priority: None,
            subject: "Which port?".into(),
            body: String::new(),
            payload: None,
            reply_to: None,
            request_id: Some("r1".into()),
        });
        let text = serde_json::to_string(&request).expect("json");
        assert_eq!(
            serde_json::from_str::<MailRequest>(&text).expect("back"),
            request
        );
        let answer = MailResponse::Error {
            message: "no such task".into(),
        };
        let text = serde_json::to_string(&answer).expect("json");
        assert_eq!(
            serde_json::from_str::<MailResponse>(&text).expect("back"),
            answer
        );
    }
}
