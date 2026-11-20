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
use brachio_runtime::mailbox::{self, MailRequest, MailResponse};
use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::{Listener, Stream};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;

use crate::child_task_request::{ChildRequest, ChildTasks};

/// How long a hook may take to send its question once connected.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const REVIEW_PROMPT_CHARS: usize = 2000;

/// A question the user is being asked.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// Identifies the question in [`Approvals::answer`].
    pub id: u64,
    /// The terminal the agent runs in, such as `<task id>:agent`.
    pub key: String,
    /// The complete tool name and arguments, ready to display as text.
    pub review: String,
}

/// What the interface needs to hear about.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// A question is waiting for the user.
    Asked(Question),
    /// A question is over, answered or not, and its bar should go away.
    Closed(u64),
}

type Delivered = Box<dyn Fn(&[String]) + Send + Sync>;

/// The open questions, and who to tell about changes.
pub struct Approvals {
    pending: Mutex<HashMap<u64, oneshot::Sender<Decision>>>,
    next: AtomicU64,
    knows: Box<dyn Fn(&str) -> bool + Send + Sync>,
    notify: Box<dyn Fn(Notice) + Send + Sync>,
    mail: Box<dyn Fn(MailRequest) -> MailResponse + Send + Sync>,
    delivered: Delivered,
    children: Box<dyn ChildTasks>,
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
    /// answer carried once that answer was written to the client, so the caller marks them
    /// read. A write that succeeds only means the bytes were handed to the operating system:
    /// delivery is at least once, so a client that dies right after may see them again, but they
    /// are never lost to a client that had already gone.
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
            next: AtomicU64::new(1),
            knows: Box::new(knows),
            notify: Box::new(notify),
            mail: Box::new(mail),
            children: Box::new(children),
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
                } => {
                    let request = ChildRequest {
                        key,
                        title,
                        prompt,
                        agent,
                        base,
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
                if !ids.is_empty() {
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
        if let Some(decision) = self.ask_user(&mut reader, request.key, review).await? {
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
    ) -> Result<Option<Decision>> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("approval list lock poisoned"))?
            .insert(id, sender);
        (self.notify)(Notice::Asked(Question { id, key, review }));
        let outcome = tokio::select! {
            decision = receiver => decision.ok(),
            // The hook closed its end: answered in the terminal, or it gave up waiting.
            _ = reader.fill_buf() => None,
        };
        if let Ok(mut map) = self.pending.lock() {
            map.remove(&id);
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
                "prompt": request.prompt.chars().take(REVIEW_PROMPT_CHARS).collect::<String>(),
            }))
            .unwrap_or_default()
        );
        let key = request.key.clone();
        match self.ask_user(reader, key, review).await {
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
struct NoChildTasks;

#[cfg(test)]
impl ChildTasks for NoChildTasks {
    fn check(&self, _: &ChildRequest) -> Result<String, String> {
        Err("Creating tasks is not available.".to_owned())
    }

    fn create(&self, _: String, _: ChildRequest) -> crate::child_task_request::CreateFuture {
        Box::pin(async { Err("Creating tasks is not available.".to_owned()) })
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

    use super::{Approvals, Notice};
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

    /// A socket over a real store holding two unread messages. The first answer is slow, so a
    /// client can leave before it is written.
    fn mailbox_over_store(name: &str) -> Store {
        use std::sync::atomic::{AtomicBool, Ordering};

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
        let slow = AtomicBool::new(true);
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
                if slow.swap(false, Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(400));
                }
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

    #[test]
    fn messages_stay_unread_when_the_client_leaves_before_the_answer() {
        tauri::async_runtime::block_on(async {
            use brachio_runtime::mailbox::MailResponse;
            use brachio_runtime::wire::{write_message, MAX_MESSAGE};

            let name = format!("ack-{}", &uuid::Uuid::new_v4().simple().to_string()[..10]);
            let store = mailbox_over_store(&name);

            let mut stream = transport::connect(&name).await.expect("connect");
            write_message(&mut stream, &check_request(), MAX_MESSAGE)
                .await
                .expect("send");
            drop(stream);
            tokio::time::sleep(Duration::from_millis(900)).await;
            assert_eq!(unread_ids(&store), ["m1", "m2"]);

            let answer = brachio_runtime::mailbox::call(&name, &check_request())
                .await
                .expect("answer");
            assert!(
                matches!(answer, MailResponse::Messages { ref messages, .. } if messages.len() == 2)
            );
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
                Notice::Closed(_) => None,
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
}
