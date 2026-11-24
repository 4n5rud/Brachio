//! Receives permission questions from agents' hooks and hands the user's answers back.
//!
//! A question stays open for as long as the hook's connection does. When the connection ends
//! first, because the user answered in the terminal or the hook gave up, the question is closed
//! and a late click is ignored.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use brachio_agent::PolicyDecision;
use brachio_runtime::approval::{ApprovalRequest, Decision};
use brachio_runtime::hook_token;
use brachio_runtime::mailbox::{self, MailRequest, MailResponse};
use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::{Listener, Stream};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;

use crate::always_allow::{self, AlwaysAllow, Candidate, Rule};
use crate::child_task_request::{ChildRequest, ChildTasks};

/// How long a hook may take to send its question once connected.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const REVIEW_PROMPT_CHARS: usize = 2000;

/// The tool and its main input, for the Activity history.
fn policy_summary(tool: &str, input: &Value) -> String {
    let target = [
        "command",
        "file_path",
        "path",
        "notebook_path",
        "url",
        "pattern",
    ]
    .iter()
    .find_map(|field| input.get(field)?.as_str())
    .unwrap_or("");
    format!("{tool}: {target}")
}

/// True when the client sends back an acknowledgement of exactly `ids` in time.
///
/// A write to a closed connection can still succeed into the operating system's buffer, so only
/// this answer proves the client received the messages. An older `brachio` never acknowledges:
/// its messages stay unread and are shown again, which is a duplicate, never a loss.
async fn acknowledged(reader: &mut BufReader<Stream>, ids: &[String]) -> bool {
    let Ok(Ok(Some(message))) =
        tokio::time::timeout(mailbox::ACK_TIMEOUT, read_message(reader, MAX_MESSAGE)).await
    else {
        return false;
    };
    let Ok(MailRequest::Ack { ids: acked }) = serde_json::from_slice(&message) else {
        return false;
    };
    let mut acked = acked;
    let mut expected = ids.to_vec();
    acked.sort();
    expected.sort();
    acked == expected
}

/// Why in-app approvals are off, when they are. Agents then ask in their own terminal, so nothing
/// is approved without the person; this lets the window say so.
#[derive(Default)]
pub struct ApprovalsOff(Mutex<Option<&'static str>>);

impl ApprovalsOff {
    /// Records the reason: `"token"` (the token file could not be written) or `"socket"` (the
    /// endpoint could not be taken).
    pub fn set(&self, reason: &'static str) {
        if let Ok(mut current) = self.0.lock() {
            *current = Some(reason);
        }
    }

    pub fn get(&self) -> Option<&'static str> {
        self.0.lock().ok().and_then(|current| *current)
    }
}

/// The token the hook socket will check, or `"token"` when it could not be issued. The socket is
/// served only with a token: a failed issue must not leave it answering connections unchecked.
pub fn token_or_off<E>(issued: Result<String, E>) -> Result<String, &'static str> {
    issued.map_err(|_| "token")
}

/// Why in-app approvals are off, or nothing when they are on.
#[tauri::command]
pub fn approvals_off(state: tauri::State<'_, Arc<ApprovalsOff>>) -> Option<&'static str> {
    state.get()
}

/// Who raised a question. Only a tool request can be remembered with "always allow".
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    Tool,
    QaRun,
    CreateTask,
}

/// A question the user is being asked.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub kind: QuestionKind,
    /// "Always allow in this task" is offered for this question.
    pub can_always_allow: bool,
    /// The leading words "always allow" could cover instead of the exact command, such as
    /// `gh pr merge`; `None` when only the exact request can be remembered.
    pub always_prefix: Option<String>,
    /// Identifies the question in [`Approvals::answer`].
    pub id: u64,
    /// The terminal the agent runs in, such as `<task id>:agent`.
    pub key: String,
    /// The complete tool name and arguments, ready to display as text.
    pub review: String,
    /// Why the project's permission rules were not used for this question, when they were not.
    pub policy_note: Option<String>,
}

/// What the project's `[permissions]` say about a request, or why they cannot be used.
pub enum PolicyAnswer {
    Decided(PolicyDecision),
    /// The rules could not be read; the user is asked, and told why.
    Unusable(String),
}

type PolicyLookup = Box<dyn Fn(&str, &str, &Value) -> PolicyAnswer + Send + Sync>;

/// What the interface needs to hear about.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// A question is waiting for the user.
    Asked(Question),
    /// A question is over, answered or not, and its bar should go away.
    Closed(u64),
    /// The hook hung up before the user answered in the app, sent before `Closed`. The user
    /// answered in the terminal, or the agent stopped waiting: the hook cannot say which.
    HungUp(u64),
    /// A remembered rule allowed a request without asking.
    AutoAllowed { key: String, summary: String },
    /// The project's `[permissions]` answered a request without asking.
    PolicyDecided {
        key: String,
        summary: String,
        allowed: bool,
        rule: String,
    },
}

struct Pending {
    question: Question,
    sender: oneshot::Sender<Decision>,
    remember: Option<Candidate>,
}

type Delivered = Box<dyn Fn(&[String]) + Send + Sync>;

/// The open questions, and who to tell about changes.
pub struct Approvals {
    pending: Mutex<HashMap<u64, Pending>>,
    always: AlwaysAllow,
    next: AtomicU64,
    knows: Box<dyn Fn(&str) -> bool + Send + Sync>,
    notify: Box<dyn Fn(Notice) + Send + Sync>,
    mail: Box<dyn Fn(MailRequest) -> MailResponse + Send + Sync>,
    delivered: Delivered,
    children: Box<dyn ChildTasks>,
    /// When set, a connection must open with this token or it is dropped unanswered.
    token: OnceLock<String>,
    policy: OnceLock<PolicyLookup>,
}

impl Approvals {
    /// `knows` says whether a terminal key belongs to a task of this app; questions from any
    /// other key are dropped. `notify` is called for every change.
    #[cfg(test)]
    pub fn new(
        knows: impl Fn(&str) -> bool + Send + Sync + 'static,
        notify: impl Fn(Notice) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Self::with_mail(
            knows,
            notify,
            |_| MailResponse::Error {
                message: "Messages are not available.".to_owned(),
            },
            NoChildTasks,
        )
    }

    /// Like [`Approvals::new`], and `mail` answers the message requests that arrive on the same
    /// socket. `children` checks and carries out requests to create tasks, which wait for the
    /// user like a permission question.
    #[cfg(test)]
    pub fn with_mail(
        knows: impl Fn(&str) -> bool + Send + Sync + 'static,
        notify: impl Fn(Notice) + Send + Sync + 'static,
        mail: impl Fn(MailRequest) -> MailResponse + Send + Sync + 'static,
        children: impl ChildTasks + 'static,
    ) -> Arc<Self> {
        Self::with_delivery(knows, notify, mail, |_| {}, children)
    }

    /// Like [`Approvals::with_mail`], and `delivered` is given the ids of the messages a `check`
    /// answer carried once the client acknowledged exactly those ids, so the caller marks them
    /// read. A write that succeeds only means the bytes were handed to the operating system, so
    /// without the acknowledgement they stay unread: delivery is at least once, a duplicate is
    /// possible and a loss is not.
    pub fn with_delivery(
        knows: impl Fn(&str) -> bool + Send + Sync + 'static,
        notify: impl Fn(Notice) + Send + Sync + 'static,
        mail: impl Fn(MailRequest) -> MailResponse + Send + Sync + 'static,
        delivered: impl Fn(&[String]) + Send + Sync + 'static,
        children: impl ChildTasks + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            delivered: Box::new(delivered),
            pending: Mutex::default(),
            always: AlwaysAllow::default(),
            next: AtomicU64::new(1),
            knows: Box::new(knows),
            notify: Box::new(notify),
            mail: Box::new(mail),
            children: Box::new(children),
            token: OnceLock::new(),
            policy: OnceLock::new(),
        })
    }

    /// Makes every connection prove it knows `token` before its request is read. Set once, before
    /// the socket is served.
    pub fn require_token(&self, token: String) {
        let _ = self.token.set(token);
    }

    /// Sets how a project's `[permissions]` are found for a terminal key. Without it every
    /// request is asked. Only the first call has an effect.
    pub fn set_policy(
        &self,
        lookup: impl Fn(&str, &str, &Value) -> PolicyAnswer + Send + Sync + 'static,
    ) {
        let _ = self.policy.set(Box::new(lookup));
    }

    /// Passes the user's choice to the waiting hook. `false` when the question is already over.
    pub fn answer(&self, id: u64, decision: Decision) -> bool {
        let sender = self.pending.lock().ok().and_then(|mut map| map.remove(&id));
        sender.is_some_and(|pending| pending.sender.send(decision).is_ok())
    }

    /// Allows an open tool request and remembers it for its task. `false` when the question is
    /// already over or cannot be remembered, in which case nothing is answered.
    pub fn answer_always(&self, id: u64) -> bool {
        self.answer_always_as(id, false)
    }

    /// Like [`Self::answer_always`], but remembers the command's leading words instead of the
    /// exact command. `false`, with nothing answered, when the request has no prefix to offer.
    pub fn answer_always_prefix(&self, id: u64) -> bool {
        self.answer_always_as(id, true)
    }

    fn answer_always_as(&self, id: u64, prefix: bool) -> bool {
        let Ok(mut map) = self.pending.lock() else {
            return false;
        };
        if map.get(&id).is_none_or(|pending| {
            pending
                .remember
                .as_ref()
                .is_none_or(|candidate| prefix && candidate.prefix_label().is_none())
        }) {
            return false;
        }
        let Some(pending) = map.remove(&id) else {
            return false;
        };
        drop(map);
        if let Some(candidate) = &pending.remember {
            let task = always_allow::task_of(&pending.question.key);
            if prefix {
                self.always.add_prefix(task, candidate);
            } else {
                self.always.add(task, candidate);
            }
        }
        pending.sender.send(Decision::Allow).is_ok()
    }

    pub fn always_rules(&self, task_id: &str) -> Vec<Rule> {
        self.always.list(task_id)
    }

    pub fn revoke_always(&self, task_id: &str, rule_id: u64) -> bool {
        self.always.revoke(task_id, rule_id)
    }

    /// The questions still waiting, oldest first, for an interface that missed their `Asked`
    /// notice.
    pub fn pending_questions(&self) -> Vec<Question> {
        let mut questions: Vec<Question> = self
            .pending
            .lock()
            .map(|map| {
                map.values()
                    .map(|pending| pending.question.clone())
                    .collect()
            })
            .unwrap_or_default();
        questions.sort_by_key(|question| question.id);
        questions
    }

    /// Shows the user a question raised by the app itself and blocks until they answer. Call it
    /// off the async runtime. `None` when the question could not be shown.
    pub fn ask_blocking(&self, key: String, review: String) -> Option<Decision> {
        let (id, receiver) = self
            .open(key, review, QuestionKind::QaRun, None, None)
            .ok()?;
        let decision = receiver.blocking_recv().ok();
        if let Ok(mut map) = self.pending.lock() {
            map.remove(&id);
        }
        (self.notify)(Notice::Closed(id));
        decision
    }

    fn open(
        &self,
        key: String,
        review: String,
        kind: QuestionKind,
        remember: Option<Candidate>,
        policy_note: Option<String>,
    ) -> Result<(u64, oneshot::Receiver<Decision>)> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        let question = Question {
            kind,
            can_always_allow: remember.is_some(),
            always_prefix: remember.as_ref().and_then(Candidate::prefix_label),
            id,
            key,
            review,
            policy_note,
        };
        self.pending
            .lock()
            .map_err(|_| anyhow!("approval list lock poisoned"))?
            .insert(
                id,
                Pending {
                    question: question.clone(),
                    sender,
                    remember,
                },
            );
        (self.notify)(Notice::Asked(question));
        Ok((id, receiver))
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
        if let Some(token) = self.token.get() {
            let proven = tokio::time::timeout(READ_TIMEOUT, hook_token::verify(&mut reader, token))
                .await
                .unwrap_or(false);
            if !proven {
                return Err(anyhow!("connection did not present the hook token"));
            }
        }
        let message = tokio::time::timeout(READ_TIMEOUT, read_message(&mut reader, MAX_MESSAGE))
            .await??
            .ok_or_else(|| anyhow!("connection closed before a question"))?;
        let value: Value = serde_json::from_slice(&message)?;
        if mailbox::is_mail(&value) {
            let request: MailRequest = serde_json::from_value(value)?;
            let consuming = matches!(request, MailRequest::Check { peek: false, .. });
            let response = match request {
                MailRequest::CreateTask {
                    key,
                    title,
                    prompt,
                    agent,
                    base,
                    after,
                } => {
                    let request = ChildRequest {
                        key,
                        title,
                        prompt,
                        agent,
                        base,
                        after,
                    };
                    self.create_task(&mut reader, request).await
                }
                MailRequest::WaitTasks {
                    key,
                    all,
                    timeout_secs,
                } => {
                    let timeout =
                        Duration::from_secs(timeout_secs.clamp(1, mailbox::MAX_WAIT_SECS));
                    let poll = || {
                        (self.mail)(MailRequest::WaitTasks {
                            key: key.clone(),
                            all,
                            timeout_secs,
                        })
                    };
                    let waiting = crate::mailbox::wait_for_reports(
                        poll,
                        all,
                        timeout,
                        crate::mailbox::WAIT_INTERVAL,
                    );
                    tokio::select! {
                        response = waiting => response,
                        // The agent was interrupted and its command closed the connection.
                        _ = reader.fill_buf() => return Ok(()),
                    }
                }
                other => (self.mail)(other),
            };
            write_message(reader.get_mut(), &response, MAX_RESPONSE).await?;
            if let (true, MailResponse::Messages { messages, .. }) = (consuming, &response) {
                let ids: Vec<String> = messages.iter().map(|message| message.id.clone()).collect();
                if !ids.is_empty() && acknowledged(&mut reader, &ids).await {
                    (self.delivered)(&ids);
                }
            }
            return Ok(());
        }
        let request: ApprovalRequest = serde_json::from_value(value)?;
        if !(self.knows)(&request.key) {
            return Ok(());
        }
        let Some(review) = request.review() else {
            return Ok(());
        };
        let input = serde_json::from_str::<Value>(&request.review_input).ok();
        let mut policy_note = None;
        if let (Some(lookup), Some(input)) = (self.policy.get(), &input) {
            match lookup(&request.key, &request.tool_name, input) {
                PolicyAnswer::Decided(PolicyDecision::Allow { rule }) => {
                    (self.notify)(Notice::PolicyDecided {
                        key: request.key.clone(),
                        summary: policy_summary(&request.tool_name, input),
                        allowed: true,
                        rule,
                    });
                    write_message(reader.get_mut(), &Decision::Allow, MAX_RESPONSE).await?;
                    return Ok(());
                }
                PolicyAnswer::Decided(PolicyDecision::Deny { rule }) => {
                    (self.notify)(Notice::PolicyDecided {
                        key: request.key.clone(),
                        summary: policy_summary(&request.tool_name, input),
                        allowed: false,
                        rule: rule.clone(),
                    });
                    let denied = Decision::Deny {
                        message: format!(
                            "Refused by this project's brachio.toml [permissions] ({rule})."
                        ),
                    };
                    write_message(reader.get_mut(), &denied, MAX_RESPONSE).await?;
                    return Ok(());
                }
                PolicyAnswer::Decided(PolicyDecision::Ask) => {}
                PolicyAnswer::Unusable(note) => policy_note = Some(note),
            }
        }
        let candidate = input
            .as_ref()
            .and_then(|input| always_allow::candidate(&request.tool_name, input));
        if let Some(candidate) = &candidate {
            if self
                .always
                .allows(always_allow::task_of(&request.key), candidate)
            {
                (self.notify)(Notice::AutoAllowed {
                    key: request.key,
                    summary: format!("{}: {}", candidate.tool, candidate.target),
                });
                write_message(reader.get_mut(), &Decision::Allow, MAX_RESPONSE).await?;
                return Ok(());
            }
        }
        let asked = self
            .ask_user(
                &mut reader,
                request.key,
                review,
                QuestionKind::Tool,
                candidate,
                policy_note,
            )
            .await?;
        if let Some(decision) = asked {
            write_message(reader.get_mut(), &decision, MAX_RESPONSE).await?;
        }
        Ok(())
    }

    /// Shows the user a question and waits for the answer. `None` when the client hung up first.
    async fn ask_user(
        &self,
        reader: &mut BufReader<Stream>,
        key: String,
        review: String,
        kind: QuestionKind,
        remember: Option<Candidate>,
        policy_note: Option<String>,
    ) -> Result<Option<Decision>> {
        let (id, receiver) = self.open(key, review, kind, remember, policy_note)?;
        let mut hung_up = false;
        let outcome = tokio::select! {
            decision = receiver => decision.ok(),
            // The hook closed its end: answered in the terminal, or it gave up waiting.
            _ = reader.fill_buf() => {
                hung_up = true;
                None
            }
        };
        let still_open = self
            .pending
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&id))
            .is_some();
        if hung_up && still_open {
            (self.notify)(Notice::HungUp(id));
        }
        (self.notify)(Notice::Closed(id));
        Ok(outcome)
    }

    async fn create_task(
        &self,
        reader: &mut BufReader<Stream>,
        request: ChildRequest,
    ) -> MailResponse {
        let lead_id = match self.children.check(&request) {
            Ok(id) => id,
            Err(message) => return MailResponse::Error { message },
        };
        let review = format!(
            "Create task\n{}",
            serde_json::to_string_pretty(&json!({
                "title": request.title,
                "agent": request.agent_name(),
                "base": request.base,
                "after": request.after,
                "prompt": request.prompt.chars().take(REVIEW_PROMPT_CHARS).collect::<String>(),
            }))
            .unwrap_or_default()
        );
        let key = request.key.clone();
        match self
            .ask_user(reader, key, review, QuestionKind::CreateTask, None, None)
            .await
        {
            Ok(Some(Decision::Allow)) => {}
            Ok(_) | Err(_) => {
                return MailResponse::Error {
                    message: "The user did not allow creating the task.".to_owned(),
                }
            }
        }
        match self.children.create(lead_id, request).await {
            Ok(child) => MailResponse::TaskCreated {
                key: child.key,
                title: child.title,
                branch: child.branch,
            },
            Err(message) => MailResponse::Error { message },
        }
    }
}

/// Used where creating tasks is not wired up.
#[cfg(test)]
pub(crate) struct NoChildTasks;

#[cfg(test)]
impl ChildTasks for NoChildTasks {
    fn check(&self, _: &ChildRequest) -> Result<String, String> {
        Err("Creating tasks is not available.".to_owned())
    }

    fn create(&self, _: String, _: ChildRequest) -> crate::child_task_request::CreateFuture {
        Box::pin(async { Err("Creating tasks is not available.".to_owned()) })
    }
}

/// The questions still open, so a reloaded interface can show them again.
#[tauri::command]
pub fn pending_approvals(approvals: tauri::State<'_, Arc<Approvals>>) -> Vec<Question> {
    approvals.pending_questions()
}

/// Allows an open tool request and remembers it for the task until the app closes.
#[tauri::command]
pub fn approval_answer_always(
    approvals: tauri::State<'_, Arc<Approvals>>,
    id: u64,
    prefix: Option<bool>,
) -> bool {
    if prefix == Some(true) {
        approvals.answer_always_prefix(id)
    } else {
        approvals.answer_always(id)
    }
}

/// The remembered answers of a task, for its Inspector.
#[tauri::command]
pub fn always_allow_list(
    approvals: tauri::State<'_, Arc<Approvals>>,
    task_id: String,
) -> Vec<Rule> {
    approvals.always_rules(&task_id)
}

/// Forgets one remembered answer, so the request is asked again.
#[tauri::command]
pub fn always_allow_revoke(
    approvals: tauri::State<'_, Arc<Approvals>>,
    task_id: String,
    rule_id: u64,
) -> bool {
    approvals.revoke_always(&task_id, rule_id)
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
    use brachio_runtime::wire::{write_message, MAX_MESSAGE};
    use serde_json::json;

    use super::{token_or_off, Approvals, ApprovalsOff, BufReader, Notice, Stream};
    use crate::child_task_request::{ChildRequest, ChildTask, ChildTasks, CreateFuture};

    fn request(key: &str) -> ApprovalRequest {
        ApprovalRequest {
            key: key.to_owned(),
            session_id: "s".to_owned(),
            tool_name: "Bash".to_owned(),
            review_input: r#"{"command":"pnpm add zod"}"#.to_owned(),
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
                    _ => None,
                });
            if let Some(id) = asked {
                return id;
            }
            assert!(Instant::now() < deadline, "no question arrived");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[test]
    fn a_question_the_interface_missed_can_be_listed_and_answered_until_it_closes() {
        tauri::async_runtime::block_on(async {
            let (name, approvals, notices) = start("task1:agent");
            let waiting = tokio::spawn({
                let name = name.clone();
                async move {
                    approval::ask(&name, &request("task1:agent"), Duration::from_secs(10)).await
                }
            });
            let id = first_question(&notices).await;
            let listed = approvals.pending_questions();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].id, id);
            assert_eq!(listed[0].key, "task1:agent");
            assert!(approvals.answer(id, Decision::Allow));
            assert_eq!(waiting.await.expect("hook task"), Some(Decision::Allow));
            assert!(approvals.pending_questions().is_empty());
        });
    }

    #[test]
    fn a_connection_without_the_token_is_dropped_and_one_with_it_is_asked() {
        tauri::async_runtime::block_on(async {
            let (name, approvals, notices) = start("task1:agent");
            approvals.require_token("secret".to_owned());
            let refused =
                approval::ask(&name, &request("task1:agent"), Duration::from_secs(3)).await;
            assert_eq!(refused, None);
            assert!(notices.lock().expect("lock").is_empty());

            // The app may close the connection before these writes finish.
            let mut stream = transport::connect(&name).await.expect("connect");
            let _ =
                write_message(&mut stream, &json!({ "hook_token": "wrong" }), MAX_MESSAGE).await;
            let _ = write_message(&mut stream, &request("task1:agent"), MAX_MESSAGE).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(notices.lock().expect("lock").is_empty());

            let mut stream = transport::connect(&name).await.expect("connect");
            write_message(&mut stream, &json!({ "hook_token": "secret" }), MAX_MESSAGE)
                .await
                .expect("send hello");
            write_message(&mut stream, &request("task1:agent"), MAX_MESSAGE)
                .await
                .expect("send request");
            let id = first_question(&notices).await;
            assert!(approvals.answer(id, Decision::Allow));
        });
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

    fn bash(key: &str, command: &str) -> ApprovalRequest {
        let mut question = request(key);
        question.review_input = serde_json::json!({ "command": command }).to_string();
        question
    }

    fn asked_count(notices: &Arc<Mutex<Vec<Notice>>>) -> usize {
        notices
            .lock()
            .expect("lock")
            .iter()
            .filter(|notice| matches!(notice, Notice::Asked(_)))
            .count()
    }

    #[test]
    fn a_project_rule_answers_without_a_question_and_is_logged_while_others_are_asked() {
        tauri::async_runtime::block_on(async {
            let (name, approvals, notices) = start("task1:agent");
            approvals.set_policy(|_, tool, _| match tool {
                "Read" => super::PolicyAnswer::Decided(brachio_agent::PolicyDecision::Allow {
                    rule: "allow = \"Read\"".to_owned(),
                }),
                "WebFetch" => super::PolicyAnswer::Decided(brachio_agent::PolicyDecision::Deny {
                    rule: "deny = \"WebFetch\"".to_owned(),
                }),
                _ => super::PolicyAnswer::Decided(brachio_agent::PolicyDecision::Ask),
            });
            let mut read = request("task1:agent");
            read.tool_name = "Read".to_owned();
            read.review_input = r#"{"file_path":"a.txt"}"#.to_owned();
            let allowed = approval::ask(&name, &read, Duration::from_secs(5)).await;
            assert_eq!(allowed, Some(Decision::Allow));

            let mut fetch = request("task1:agent");
            fetch.tool_name = "WebFetch".to_owned();
            fetch.review_input = r#"{"url":"https://example.com"}"#.to_owned();
            let refused = approval::ask(&name, &fetch, Duration::from_secs(5)).await;
            assert!(
                matches!(refused, Some(Decision::Deny { .. })),
                "{refused:?}"
            );

            assert_eq!(asked_count(&notices), 0, "no bar for a decided request");
            let decided: Vec<bool> = notices
                .lock()
                .expect("lock")
                .iter()
                .filter_map(|notice| match notice {
                    Notice::PolicyDecided { allowed, .. } => Some(*allowed),
                    _ => None,
                })
                .collect();
            assert_eq!(decided, vec![true, false]);

            let waiting = tokio::spawn({
                let name = name.clone();
                async move {
                    approval::ask(&name, &request("task1:agent"), Duration::from_secs(5)).await
                }
            });
            let id = first_question(&notices).await;
            assert!(approvals.answer(id, Decision::Allow));
            assert_eq!(waiting.await.expect("hook task"), Some(Decision::Allow));
        });
    }

    #[test]
    fn an_always_allowed_command_is_allowed_by_the_app_and_logged_but_others_are_asked() {
        tauri::async_runtime::block_on(async {
            let (name, approvals, notices) = start("task1:agent");
            let first = tokio::spawn({
                let name = name.clone();
                async move {
                    approval::ask(
                        &name,
                        &bash("task1:agent", "cargo test"),
                        Duration::from_secs(10),
                    )
                    .await
                }
            });
            let id = first_question(&notices).await;
            assert!(approvals.answer_always(id));
            assert_eq!(first.await.expect("hook task"), Some(Decision::Allow));

            let again = approval::ask(
                &name,
                &bash("task1:agent", "cargo test"),
                Duration::from_secs(5),
            )
            .await;
            assert_eq!(again, Some(Decision::Allow));
            assert_eq!(asked_count(&notices), 1, "the repeat was not asked");
            assert!(notices
                .lock()
                .expect("lock")
                .contains(&Notice::AutoAllowed {
                    key: "task1:agent".to_owned(),
                    summary: "Bash: cargo test".to_owned(),
                }));

            let other = approval::ask(
                &name,
                &bash("task1:agent", "cargo test --all"),
                Duration::from_millis(300),
            )
            .await;
            assert_eq!(other, None);
            assert_eq!(asked_count(&notices), 2, "another command is asked");
        });
    }

    #[test]
    fn a_rule_of_one_task_does_not_allow_another_task_and_revoking_asks_again() {
        tauri::async_runtime::block_on(async {
            let name = format!(
                "approvals-{}",
                &uuid::Uuid::new_v4().simple().to_string()[..10]
            );
            let notices = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&notices);
            let approvals = Approvals::new(
                |_| true,
                move |notice| sink.lock().expect("lock").push(notice),
            );
            let listener = transport::listen(&name).expect("listen");
            tauri::async_runtime::spawn(Arc::clone(&approvals).serve(listener));
            let first = tokio::spawn({
                let name = name.clone();
                async move {
                    approval::ask(&name, &bash("a:agent", "ls"), Duration::from_secs(10)).await
                }
            });
            let id = first_question(&notices).await;
            assert!(approvals.answer_always(id));
            first.await.expect("hook task");

            let other_task =
                approval::ask(&name, &bash("b:agent", "ls"), Duration::from_millis(300)).await;
            assert_eq!(other_task, None, "task b has no rule");
            let rules = approvals.always_rules("a");
            assert_eq!(rules.len(), 1);
            assert!(approvals.revoke_always("a", rules[0].id));
            let revoked =
                approval::ask(&name, &bash("a:agent", "ls"), Duration::from_millis(300)).await;
            assert_eq!(revoked, None, "a revoked rule asks again");
        });
    }

    #[test]
    fn qa_runs_and_task_creation_can_never_be_remembered() {
        let approvals = Approvals::new(|_| true, |_| {});
        let asking = Arc::clone(&approvals);
        let thread =
            std::thread::spawn(move || asking.ask_blocking("t:agent".into(), "npm test".into()));
        let id = loop {
            if let Some(question) = approvals.pending_questions().first() {
                assert_eq!(question.kind, super::QuestionKind::QaRun);
                assert!(!question.can_always_allow);
                break question.id;
            }
            std::thread::yield_now();
        };
        assert!(!approvals.answer_always(id));
        assert!(approvals.always_rules("t").is_empty());
        assert!(approvals.answer(id, Decision::Allow));
        thread.join().expect("join");
    }

    #[test]
    fn a_hook_that_hangs_up_is_reported_before_its_question_closes() {
        tauri::async_runtime::block_on(async {
            let (name, _approvals, notices) = start("task1:agent");
            let question = request("task1:agent");
            let waiting = approval::ask(&name, &question, Duration::from_millis(300));
            assert_eq!(waiting.await, None);
            let id = first_question(&notices).await;
            let deadline = Instant::now() + Duration::from_secs(5);
            while !notices.lock().expect("lock").contains(&Notice::Closed(id)) {
                assert!(Instant::now() < deadline, "question stayed open");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let seen = notices.lock().expect("lock").clone();
            let hung = seen.iter().position(|n| *n == Notice::HungUp(id));
            let closed = seen.iter().position(|n| *n == Notice::Closed(id));
            assert!(hung.is_some() && hung < closed);
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

    #[test]
    fn unreviewable_input_never_creates_a_pending_approval() {
        tauri::async_runtime::block_on(async {
            let (name, approvals, notices) = start("task1:agent");
            let mut question = request("task1:agent");
            question.review_input = format!(r#"{{"content":"{}"}}"#, "x".repeat(3_000));
            let answer = approval::ask(&name, &question, Duration::from_secs(5)).await;
            assert_eq!(answer, None);
            assert!(notices.lock().expect("lock").is_empty());
            assert!(!approvals.answer(1, Decision::Allow));
        });
    }

    #[test]
    fn message_requests_on_the_same_socket_reach_the_mail_handler() {
        tauri::async_runtime::block_on(async {
            use brachio_runtime::mailbox::{self, MailRequest, MailResponse};

            let name = format!("mail-{}", &uuid::Uuid::new_v4().simple().to_string()[..10]);
            let approvals = Approvals::with_mail(
                |_| false,
                |_| {},
                |request| match request {
                    MailRequest::Peers { key } => MailResponse::Error {
                        message: format!("asked by {key}"),
                    },
                    _ => MailResponse::Error {
                        message: "other".to_owned(),
                    },
                },
                super::NoChildTasks,
            );
            let listener = transport::listen(&name).expect("listen");
            tauri::async_runtime::spawn(approvals.serve(listener));

            let answer = mailbox::call(
                &name,
                &MailRequest::Peers {
                    key: "t:agent".to_owned(),
                },
            )
            .await
            .expect("answer");
            assert_eq!(
                answer,
                MailResponse::Error {
                    message: "asked by t:agent".to_owned()
                }
            );
        });
    }

    fn check_request() -> brachio_runtime::mailbox::MailRequest {
        brachio_runtime::mailbox::MailRequest::Check {
            key: "t:agent".to_owned(),
            limit: None,
            peek: false,
        }
    }

    type Store = Arc<Mutex<rusqlite::Connection>>;
    /// A socket over a real store holding two unread messages for `t:agent`.
    fn mailbox_over_store(name: &str) -> Store {
        use brachio_runtime::mailbox::{MailMessage, MailRequest, MailResponse};

        let store = Arc::new(Mutex::new(brachio_store::open_in_memory().expect("open")));
        {
            let connection = store.lock().expect("lock");
            for (id, at) in [("m1", 1), ("m2", 2)] {
                brachio_store::send_message(
                    &connection,
                    &brachio_store::NewMessage {
                        id,
                        from_key: "s:agent",
                        to_key: "t:agent",
                        kind: "status",
                        priority: "normal",
                        subject: "hi",
                        body: "",
                        payload_json: None,
                        reply_to: None,
                        request_id: None,
                        created_at_ms: at,
                    },
                )
                .expect("send");
            }
        }
        let reading = Arc::clone(&store);
        let marking = Arc::clone(&store);
        let approvals = Approvals::with_delivery(
            |_| false,
            |_| {},
            move |request| {
                let MailRequest::Check { key, .. } = request else {
                    unreachable!("only check is used");
                };
                let messages = {
                    let connection = reading.lock().expect("lock");
                    brachio_store::peek_next_batch(&connection, &key, 10).expect("peek")
                };
                MailResponse::Messages {
                    messages: messages
                        .into_iter()
                        .map(|message| MailMessage {
                            id: message.id,
                            thread_id: message.thread_id,
                            from: message.from_key,
                            to: message.to_key,
                            from_title: None,
                            kind: message.kind,
                            priority: message.priority,
                            subject: message.subject,
                            body: message.body,
                            payload: None,
                            created_at_ms: message.created_at_ms,
                            read: false,
                        })
                        .collect(),
                    unread_left: 0,
                }
            },
            move |ids| {
                let mut connection = marking.lock().expect("lock");
                brachio_store::mark_read(&mut connection, ids, 50).expect("mark");
            },
            super::NoChildTasks,
        );
        let listener = transport::listen(name).expect("listen");
        tauri::async_runtime::spawn(approvals.serve(listener));
        store
    }

    fn unread_ids(store: &Store) -> Vec<String> {
        let connection = store.lock().expect("lock");
        brachio_store::peek_unread(&connection, "t:agent", 10)
            .expect("peek")
            .into_iter()
            .map(|message| message.id)
            .collect()
    }

    /// Sends a consuming check and reads the answer, leaving the connection open for an ack.
    async fn check_and_receive(name: &str) -> BufReader<Stream> {
        use brachio_runtime::mailbox::MailResponse;
        use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};

        let mut stream = transport::connect(name).await.expect("connect");
        write_message(&mut stream, &check_request(), MAX_MESSAGE)
            .await
            .expect("send");
        let mut reader = BufReader::new(stream);
        let message = read_message(&mut reader, MAX_RESPONSE)
            .await
            .expect("read")
            .expect("an answer");
        let answer: MailResponse = serde_json::from_slice(&message).expect("decode");
        assert!(
            matches!(answer, MailResponse::Messages { ref messages, .. } if messages.len() == 2)
        );
        reader
    }

    /// Sends `ids` as the acknowledgement and waits until the server has closed the connection,
    /// which it only does after deciding what to mark.
    async fn ack_and_wait_for_close(mut reader: BufReader<Stream>, ids: &[&str]) {
        use brachio_runtime::mailbox::MailRequest;
        use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};

        let ack = MailRequest::Ack {
            ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        };
        write_message(reader.get_mut(), &ack, MAX_MESSAGE)
            .await
            .expect("ack");
        let closed = tokio::time::timeout(
            Duration::from_secs(30),
            read_message(&mut reader, MAX_RESPONSE),
        )
        .await
        .expect("the server never finished");
        assert!(!matches!(closed, Ok(Some(_))));
    }

    fn unique_name() -> String {
        format!("ack-{}", &uuid::Uuid::new_v4().simple().to_string()[..10])
    }

    #[test]
    fn messages_stay_unread_when_the_client_leaves_without_acknowledging() {
        tauri::async_runtime::block_on(async {
            let name = unique_name();
            let store = mailbox_over_store(&name);

            drop(check_and_receive(&name).await);
            let again = check_and_receive(&name).await;
            assert_eq!(unread_ids(&store), ["m1", "m2"]);
            drop(again);
        });
    }

    #[test]
    fn messages_stay_unread_when_the_acknowledgement_names_other_ids() {
        tauri::async_runtime::block_on(async {
            let name = unique_name();
            let store = mailbox_over_store(&name);

            ack_and_wait_for_close(check_and_receive(&name).await, &["m1", "other"]).await;
            assert_eq!(unread_ids(&store), ["m1", "m2"]);
            ack_and_wait_for_close(check_and_receive(&name).await, &["m1"]).await;
            assert_eq!(unread_ids(&store), ["m1", "m2"]);
        });
    }

    #[test]
    fn messages_are_marked_read_once_the_client_acknowledges_them() {
        tauri::async_runtime::block_on(async {
            let name = unique_name();
            let store = mailbox_over_store(&name);

            ack_and_wait_for_close(check_and_receive(&name).await, &["m2", "m1"]).await;
            assert!(unread_ids(&store).is_empty());
        });
    }

    #[test]
    fn the_cli_helper_acknowledges_after_showing_the_messages() {
        tauri::async_runtime::block_on(async {
            use brachio_runtime::mailbox::MailResponse;

            let name = unique_name();
            let store = mailbox_over_store(&name);

            let shown = std::cell::Cell::new(0);
            brachio_runtime::mailbox::call_with_ack(
                &name,
                &check_request(),
                Duration::from_secs(30),
                |answer| {
                    if let MailResponse::Messages { messages, .. } = answer {
                        shown.set(messages.len());
                    }
                    Ok(())
                },
            )
            .await
            .expect("check");
            assert_eq!(shown.get(), 2);
            assert!(unread_ids(&store).is_empty());
        });
    }

    #[test]
    fn a_wait_stops_looking_once_the_command_hangs_up() {
        tauri::async_runtime::block_on(async {
            use std::sync::atomic::{AtomicU32, Ordering};

            use brachio_runtime::mailbox::{MailRequest, MailResponse};
            use brachio_runtime::wire::{write_message, MAX_MESSAGE};

            let name = format!("wait-{}", &uuid::Uuid::new_v4().simple().to_string()[..10]);
            let polls = Arc::new(AtomicU32::new(0));
            let counted = Arc::clone(&polls);
            let approvals = Approvals::with_mail(
                |_| false,
                |_| {},
                move |_| {
                    counted.fetch_add(1, Ordering::SeqCst);
                    MailResponse::TasksReported {
                        reported: vec![],
                        pending: vec!["t:agent".to_owned()],
                        timed_out: false,
                    }
                },
                super::NoChildTasks,
            );
            let listener = transport::listen(&name).expect("listen");
            tauri::async_runtime::spawn(approvals.serve(listener));

            let mut stream = transport::connect(&name).await.expect("connect");
            let request = MailRequest::WaitTasks {
                key: "lead:agent".to_owned(),
                all: true,
                timeout_secs: 600,
            };
            write_message(&mut stream, &request, MAX_MESSAGE)
                .await
                .expect("send");
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert_eq!(polls.load(Ordering::SeqCst), 1);
            drop(stream);

            tokio::time::sleep(Duration::from_millis(1500)).await;
            let after_hangup = polls.load(Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(2200)).await;
            assert_eq!(polls.load(Ordering::SeqCst), after_hangup);
        });
    }

    struct StandInChildren {
        created: Arc<Mutex<Vec<ChildRequest>>>,
    }

    impl ChildTasks for StandInChildren {
        fn check(&self, _: &ChildRequest) -> Result<String, String> {
            Ok("lead1111".to_owned())
        }

        fn create(&self, lead_id: String, request: ChildRequest) -> CreateFuture {
            assert_eq!(lead_id, "lead1111");
            self.created.lock().expect("lock").push(request);
            Box::pin(async {
                Ok(ChildTask {
                    key: "child001:agent".to_owned(),
                    title: "Write docs".to_owned(),
                    branch: "write-docs-abc123".to_owned(),
                })
            })
        }
    }

    type Notices = Arc<Mutex<Vec<Notice>>>;
    type Created = Arc<Mutex<Vec<ChildRequest>>>;

    fn start_with_children() -> (String, Arc<Approvals>, Notices, Created) {
        let name = format!("child-{}", &uuid::Uuid::new_v4().simple().to_string()[..10]);
        let notices = Arc::new(Mutex::new(Vec::new()));
        let created = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&notices);
        let approvals = Approvals::with_mail(
            |_| false,
            move |notice| sink.lock().expect("lock").push(notice),
            |_| unreachable!("create_task does not use the mail closure"),
            StandInChildren {
                created: Arc::clone(&created),
            },
        );
        let listener = transport::listen(&name).expect("listen");
        tauri::async_runtime::spawn(Arc::clone(&approvals).serve(listener));
        (name, approvals, notices, created)
    }

    fn create_request() -> brachio_runtime::mailbox::MailRequest {
        brachio_runtime::mailbox::MailRequest::CreateTask {
            key: "lead1111:agent".to_owned(),
            title: "Write docs".to_owned(),
            prompt: "Document the API".to_owned(),
            agent: Some("codex".to_owned()),
            base: None,
            after: Vec::new(),
        }
    }

    async fn asked_review(notices: &Arc<Mutex<Vec<Notice>>>) -> (u64, String) {
        let id = first_question(notices).await;
        let review = notices
            .lock()
            .expect("lock")
            .iter()
            .find_map(|notice| match notice {
                Notice::Asked(question) => Some(question.review.clone()),
                _ => None,
            })
            .expect("question");
        (id, review)
    }

    #[test]
    fn an_allowed_task_request_creates_the_task_and_returns_its_key() {
        tauri::async_runtime::block_on(async {
            use brachio_runtime::mailbox::{self, MailResponse};

            let (name, approvals, notices, created) = start_with_children();
            let waiting = tokio::spawn({
                let name = name.clone();
                async move { mailbox::call(&name, &create_request()).await }
            });
            let (id, review) = asked_review(&notices).await;
            assert!(review.starts_with("Create task\n"));
            assert!(review.contains("Document the API"));
            assert!(created.lock().expect("lock").is_empty());
            assert!(approvals.answer(id, Decision::Allow));
            assert_eq!(
                waiting.await.expect("task").expect("answer"),
                MailResponse::TaskCreated {
                    key: "child001:agent".to_owned(),
                    title: "Write docs".to_owned(),
                    branch: "write-docs-abc123".to_owned(),
                }
            );
            let made = created.lock().expect("lock");
            assert_eq!(made.len(), 1);
            assert_eq!(made[0].agent.as_deref(), Some("codex"));
        });
    }

    #[test]
    fn a_denied_task_request_is_refused_and_nothing_is_created() {
        tauri::async_runtime::block_on(async {
            use brachio_runtime::mailbox::{self, MailResponse};

            let (name, approvals, notices, created) = start_with_children();
            let waiting = tokio::spawn({
                let name = name.clone();
                async move { mailbox::call(&name, &create_request()).await }
            });
            let (id, _) = asked_review(&notices).await;
            let denied = Decision::Deny {
                message: "no".to_owned(),
            };
            assert!(approvals.answer(id, denied));
            assert_eq!(
                waiting.await.expect("task").expect("answer"),
                MailResponse::Error {
                    message: "The user did not allow creating the task.".to_owned()
                }
            );
            assert!(created.lock().expect("lock").is_empty());
        });
    }

    #[test]
    fn a_failed_token_turns_in_app_approval_off_and_says_why() {
        assert_eq!(
            token_or_off::<()>(Ok("secret".to_owned())),
            Ok("secret".to_owned())
        );
        assert_eq!(token_or_off(Err("disk full")), Err("token"));
        let off = ApprovalsOff::default();
        assert_eq!(off.get(), None);
        off.set("token");
        assert_eq!(off.get(), Some("token"));
    }
}
