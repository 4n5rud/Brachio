//! Receives permission questions from agents' hooks and hands the user's answers back.
//!
//! A question stays open for as long as the hook's connection does. When the connection ends
//! first, because the user answered in the terminal or the hook gave up, the question is closed
//! and a late click is ignored.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use brachio_runtime::approval::{ApprovalRequest, Decision};
use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::{Listener, Stream};
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;

/// How long a hook may take to send its question once connected.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// A question the user is being asked.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// Identifies the question in [`Approvals::answer`].
    pub id: u64,
    /// The terminal the agent runs in, such as `<task id>:agent`.
    pub key: String,
    /// The tool the agent wants to use.
    pub tool_name: String,
    /// The tool's arguments, shortened for display.
    pub tool_input: Value,
}

/// What the interface needs to hear about.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// A question is waiting for the user.
    Asked(Question),
    /// A question is over, answered or not, and its bar should go away.
    Closed(u64),
}

/// The open questions, and who to tell about changes.
pub struct Approvals {
    pending: Mutex<HashMap<u64, oneshot::Sender<Decision>>>,
    next: AtomicU64,
    knows: Box<dyn Fn(&str) -> bool + Send + Sync>,
    notify: Box<dyn Fn(Notice) + Send + Sync>,
}

impl Approvals {
    /// `knows` says whether a terminal key belongs to a task of this app; questions from any
    /// other key are dropped. `notify` is called for every change.
    pub fn new(
        knows: impl Fn(&str) -> bool + Send + Sync + 'static,
        notify: impl Fn(Notice) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            pending: Mutex::default(),
            next: AtomicU64::new(1),
            knows: Box::new(knows),
            notify: Box::new(notify),
        })
    }

    /// Passes the user's choice to the waiting hook. `false` when the question is already over.
    pub fn answer(&self, id: u64, decision: Decision) -> bool {
        let sender = self.pending.lock().ok().and_then(|mut map| map.remove(&id));
        sender.is_some_and(|sender| sender.send(decision).is_ok())
    }

    /// Serves hooks until the listener fails.
    pub async fn serve(self: Arc<Self>, listener: Listener) {
        while let Ok(stream) = listener.accept().await {
            let approvals = Arc::clone(&self);
            tauri::async_runtime::spawn(async move {
                if let Err(error) = approvals.handle(stream).await {
                    tracing::debug!(%error, "permission question ended");
                }
            });
        }
    }

    async fn handle(&self, stream: Stream) -> Result<()> {
        let mut reader = BufReader::new(stream);
        let message = tokio::time::timeout(READ_TIMEOUT, read_message(&mut reader, MAX_MESSAGE))
            .await??
            .ok_or_else(|| anyhow!("connection closed before a question"))?;
        let request: ApprovalRequest = serde_json::from_slice(&message)?;
        if !(self.knows)(&request.key) {
            return Ok(());
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("approval list lock poisoned"))?
            .insert(id, sender);
        (self.notify)(Notice::Asked(Question {
            id,
            key: request.key,
            tool_name: request.tool_name,
            tool_input: request.tool_input,
        }));
        let outcome = tokio::select! {
            decision = receiver => decision.ok(),
            // The hook closed its end: answered in the terminal, or it gave up waiting.
            _ = reader.fill_buf() => None,
        };
        if let Ok(mut map) = self.pending.lock() {
            map.remove(&id);
        }
        (self.notify)(Notice::Closed(id));
        if let Some(decision) = outcome {
            write_message(reader.get_mut(), &decision, MAX_RESPONSE).await?;
        }
        Ok(())
    }
}

/// The user's choice for an open question. Nothing else can answer one.
#[tauri::command]
pub fn approval_answer(approvals: tauri::State<'_, Arc<Approvals>>, id: u64, allow: bool) -> bool {
    let decision = if allow {
        Decision::Allow
    } else {
        Decision::Deny {
            message: "Denied in Brachio".to_owned(),
        }
    };
    approvals.answer(id, decision)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use brachio_runtime::approval::{self, ApprovalRequest, Decision};
    use brachio_runtime::transport;
    use serde_json::json;

    use super::{Approvals, Notice};

    fn request(key: &str) -> ApprovalRequest {
        ApprovalRequest {
            key: key.to_owned(),
            session_id: "s".to_owned(),
            tool_name: "Bash".to_owned(),
            tool_input: json!({ "command": "pnpm add zod" }),
            tool_use_id: "t".to_owned(),
        }
    }

    fn start(known: &'static str) -> (String, Arc<Approvals>, Arc<Mutex<Vec<Notice>>>) {
        let name = format!(
            "approvals-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..10]
        );
        let notices = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&notices);
        let approvals = Approvals::new(
            move |key| key == known,
            move |notice| sink.lock().expect("lock").push(notice),
        );
        let listener = transport::listen(&name).expect("listen");
        tauri::async_runtime::spawn(Arc::clone(&approvals).serve(listener));
        (name, approvals, notices)
    }

    async fn first_question(notices: &Arc<Mutex<Vec<Notice>>>) -> u64 {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let asked = notices
                .lock()
                .expect("lock")
                .iter()
                .find_map(|notice| match notice {
                    Notice::Asked(question) => Some(question.id),
                    Notice::Closed(_) => None,
                });
            if let Some(id) = asked {
                return id;
            }
            assert!(Instant::now() < deadline, "no question arrived");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[test]
    fn a_click_reaches_the_waiting_hook_and_closes_the_question() {
        tauri::async_runtime::block_on(async {
            let (name, approvals, notices) = start("task1:agent");
            let waiting = tokio::spawn({
                let name = name.clone();
                async move {
                    approval::ask(&name, &request("task1:agent"), Duration::from_secs(10)).await
                }
            });
            let id = first_question(&notices).await;
            assert!(approvals.answer(id, Decision::Allow));
            assert_eq!(waiting.await.expect("hook task"), Some(Decision::Allow));
            assert!(notices.lock().expect("lock").contains(&Notice::Closed(id)));
            assert!(
                !approvals.answer(id, Decision::Allow),
                "a second click is ignored"
            );
        });
    }

    #[test]
    fn a_hook_that_gives_up_closes_the_question_and_a_late_click_is_ignored() {
        tauri::async_runtime::block_on(async {
            let (name, approvals, notices) = start("task1:agent");
            let question = request("task1:agent");
            let waiting = approval::ask(&name, &question, Duration::from_millis(300));
            assert_eq!(waiting.await, None);
            let id = first_question(&notices).await;
            let deadline = Instant::now() + Duration::from_secs(5);
            while !notices.lock().expect("lock").contains(&Notice::Closed(id)) {
                assert!(Instant::now() < deadline, "question stayed open");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert!(!approvals.answer(id, Decision::Allow));
        });
    }

    #[test]
    fn a_question_from_a_terminal_the_app_does_not_know_is_dropped() {
        tauri::async_runtime::block_on(async {
            let (name, _approvals, notices) = start("task1:agent");
            let answer = approval::ask(
                &name,
                &request("someone-else:agent"),
                Duration::from_secs(5),
            )
            .await;
            assert_eq!(answer, None);
            assert!(notices.lock().expect("lock").is_empty());
        });
    }
}
