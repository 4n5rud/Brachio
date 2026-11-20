//! Runs the real `brachio send` and `brachio check` against a stand-in for the app.

use brachio_runtime::mailbox::{MailMessage, MailRequest, MailResponse};
use brachio_runtime::transport;
use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

const BRACHIO: &str = env!("CARGO_BIN_EXE_brachio");

/// Accepts one request, hands it back and answers with `response`.
fn app(name: &str, response: MailResponse) -> tokio::task::JoinHandle<MailRequest> {
    let listener = transport::listen(name).expect("listen");
    tokio::spawn(async move {
        use interprocess::local_socket::tokio::prelude::*;
        let stream = listener.accept().await.expect("accept");
        let mut reader = BufReader::new(stream);
        let message = read_message(&mut reader, MAX_MESSAGE)
            .await
            .expect("read")
            .expect("a request");
        let request: MailRequest = serde_json::from_slice(&message).expect("decode");
        write_message(reader.get_mut(), &response, MAX_RESPONSE)
            .await
            .expect("answer");
        let _ = reader.get_mut().shutdown().await;
        request
    })
}

async fn brachio(endpoint: Option<&str>, args: &[&str]) -> (String, String, i32) {
    let mut command = Command::new(BRACHIO);
    command
        .args(args)
        .env_remove("BRACHIO_HOOK_ENDPOINT")
        .env_remove("BRACHIO_TERMINAL_KEY");
    if let Some(endpoint) = endpoint {
        command
            .env("BRACHIO_HOOK_ENDPOINT", endpoint)
            .env("BRACHIO_TERMINAL_KEY", "task1:agent");
    }
    let output = command.output().await.expect("run brachio");
    (
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        output.status.code().unwrap_or(-1),
    )
}

fn name() -> String {
    format!("mail-{}", &Uuid::new_v4().simple().to_string()[..12])
}

#[tokio::test]
async fn send_puts_the_sender_the_receiver_and_the_text_in_one_request() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::Sent {
            id: "m1".into(),
            thread_id: "m1".into(),
            to: "task2:agent".into(),
            duplicate: false,
        },
    );
    let (out, _, code) = brachio(
        Some(&endpoint),
        &[
            "send",
            "--to",
            "Write docs",
            "--subject",
            "Port?",
            "--body",
            "which one",
            "--type",
            "question",
        ],
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(out, "sent m1 to task2:agent");
    let MailRequest::Send(sent) = served.await.expect("app task") else {
        panic!("expected a send request");
    };
    assert_eq!(sent.from, "task1:agent");
    assert_eq!(
        (sent.to.as_str(), sent.kind.as_str()),
        ("Write docs", "question")
    );
    assert_eq!(
        (sent.subject.as_str(), sent.body.as_str()),
        ("Port?", "which one")
    );
}

#[tokio::test]
async fn ask_and_reply_send_one_request_each() {
    for (args, expected_kind, expected_to, expected_parent) in [
        (
            &["ask", "--to", "task2:agent", "--subject", "Ready?"][..],
            "question",
            "task2:agent",
            None,
        ),
        (
            &["reply", "m1", "--subject", "Ready"][..],
            "status",
            "",
            Some("m1"),
        ),
    ] {
        let endpoint = name();
        let served = app(
            &endpoint,
            MailResponse::Sent {
                id: "m2".into(),
                thread_id: "m1".into(),
                to: "task2:agent".into(),
                duplicate: false,
            },
        );
        let (out, err, code) = brachio(Some(&endpoint), args).await;
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "sent m2 to task2:agent");
        let MailRequest::Send(sent) = served.await.expect("app task") else {
            panic!("expected a send request");
        };
        assert_eq!(sent.kind, expected_kind);
        assert_eq!(sent.to, expected_to);
        assert_eq!(sent.reply_to.as_deref(), expected_parent);
    }
}

#[tokio::test]
async fn check_prints_one_line_per_message_and_what_is_left() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::Messages {
            messages: vec![MailMessage {
                id: "m9".into(),
                thread_id: "m9".into(),
                from: "task2:agent".into(),
                to: "task1:agent".into(),
                from_title: Some("Write docs".into()),
                kind: "question".into(),
                priority: "high".into(),
                subject: "Which port?".into(),
                body: "dev server".into(),
                payload: None,
                created_at_ms: 0,
                read: false,
            }],
            unread_left: 2,
        },
    );
    let (out, _, code) = brachio(Some(&endpoint), &["check"]).await;
    assert_eq!(code, 0);
    assert!(
        out.starts_with("m9 [question, high] from Write docs: Which port?"),
        "{out}"
    );
    assert!(out.contains("| dev server"));
    assert!(out.ends_with("2 more unread; run brachio check again"));
    assert!(matches!(
        served.await.expect("app task"),
        MailRequest::Check { peek: false, .. }
    ));
}

async fn mail_hook(endpoint: &str, state: &std::path::Path, event: &str) -> String {
    use tokio::io::AsyncWriteExt as _;
    let mut child = Command::new(BRACHIO)
        .args(["hook", "mail"])
        .env("BRACHIO_HOOK_ENDPOINT", endpoint)
        .env("BRACHIO_TERMINAL_KEY", "task1:agent")
        .env("BRACHIO_MAIL_STATE_DIR", state)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start brachio");
    let mut stdin = child.stdin.take().expect("stdin");
    stdin
        .write_all(format!(r#"{{"hook_event_name":"{event}"}}"#).as_bytes())
        .await
        .expect("send event");
    drop(stdin);
    let output = child.wait_with_output().await.expect("finish");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[tokio::test]
async fn the_mail_hook_speaks_once_per_new_message_and_otherwise_stays_silent() {
    let state = tempfile::tempdir().expect("state dir");
    let waiting = |unread_left| MailResponse::Messages {
        messages: Vec::new(),
        unread_left,
    };

    let endpoint = name();
    let served = app(&endpoint, waiting(2));
    let first = mail_hook(&endpoint, state.path(), "PostToolUse").await;
    served.await.expect("app task");
    assert!(first.contains("\"PostToolUse\""), "{first}");
    assert!(first.contains("2 unread messages"), "{first}");

    let endpoint = name();
    let served = app(&endpoint, waiting(2));
    assert_eq!(mail_hook(&endpoint, state.path(), "PostToolUse").await, "");
    served.await.expect("app task");

    let endpoint = name();
    let served = app(&endpoint, waiting(3));
    let more = mail_hook(&endpoint, state.path(), "UserPromptSubmit").await;
    served.await.expect("app task");
    assert!(more.contains("3 unread messages"), "{more}");

    assert_eq!(
        mail_hook(&name(), state.path(), "PostToolUse").await,
        "",
        "no app, no output"
    );
}

#[tokio::test]
async fn outside_a_brachio_terminal_or_with_the_app_closed_it_says_so_and_fails() {
    let (_, error, code) = brachio(None, &["check"]).await;
    assert_ne!(code, 0);
    assert!(
        error.contains("Not running inside a Brachio agent terminal"),
        "{error}"
    );

    let (_, error, code) = brachio(Some(&name()), &["peers"]).await;
    assert_ne!(code, 0);
    assert!(error.contains("not running"), "{error}");
}

#[tokio::test]
async fn task_create_sends_the_request_and_prints_the_new_key_and_branch() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::TaskCreated {
            key: "child001:agent".into(),
            title: "Write docs".into(),
            branch: "write-docs-abc123".into(),
        },
    );
    let (out, _, code) = brachio(
        Some(&endpoint),
        &[
            "task",
            "create",
            "--title",
            "Write docs",
            "--prompt",
            "Document it",
            "--agent",
            "codex",
        ],
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(out, "created child001:agent (write-docs-abc123)");
    assert_eq!(
        served.await.expect("app task"),
        MailRequest::CreateTask {
            key: "task1:agent".into(),
            title: "Write docs".into(),
            prompt: "Document it".into(),
            agent: Some("codex".into()),
            base: None,
        }
    );
}

#[tokio::test]
async fn task_list_prints_one_line_per_child_or_says_there_are_none() {
    use brachio_runtime::mailbox::ChildTaskInfo;

    let child = |key: &str, title: &str, archived, reported, unread| ChildTaskInfo {
        key: key.into(),
        title: title.into(),
        archived,
        reported,
        unread,
    };
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::TaskList {
            children: vec![
                child("c1:agent", "Parser", false, true, 2),
                child("c2:agent", "Docs", false, false, 0),
                child("c3:agent", "Old", true, false, 0),
            ],
        },
    );
    let (out, _, code) = brachio(Some(&endpoint), &["task", "list"]).await;
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "c1:agent  Parser  open reported  unread=2\nc2:agent  Docs  open  unread=0\nc3:agent  Old  archived  unread=0"
    );
    assert_eq!(
        served.await.expect("app task"),
        MailRequest::ListTasks {
            key: "task1:agent".into()
        }
    );

    let endpoint = name();
    app(&endpoint, MailResponse::TaskList { children: vec![] });
    let (out, _, _) = brachio(Some(&endpoint), &["task", "list"]).await;
    assert_eq!(out, "no child tasks");
}

#[tokio::test]
async fn task_wait_prints_reports_and_exits_with_three_on_timeout() {
    use brachio_runtime::mailbox::ReportedChild;

    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::TasksReported {
            reported: vec![ReportedChild {
                key: "c1:agent".into(),
                subject: "done: parser".into(),
            }],
            pending: vec!["c2:agent".into(), "c3:agent".into()],
            timed_out: true,
        },
    );
    let (out, _, code) = brachio(
        Some(&endpoint),
        &["task", "wait", "--any", "--timeout-secs", "30"],
    )
    .await;
    assert_eq!(code, 3);
    assert_eq!(
        out,
        "timeout\nc1:agent done: parser\npending: c2:agent c3:agent"
    );
    assert_eq!(
        served.await.expect("app task"),
        MailRequest::WaitTasks {
            key: "task1:agent".into(),
            all: false,
            timeout_secs: 30
        }
    );
}

#[tokio::test]
async fn task_wait_defaults_to_all_children_and_prints_each_report() {
    use brachio_runtime::mailbox::ReportedChild;

    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::TasksReported {
            reported: vec![
                ReportedChild {
                    key: "c1:agent".into(),
                    subject: "one".into(),
                },
                ReportedChild {
                    key: "c2:agent".into(),
                    subject: "two".into(),
                },
            ],
            pending: vec![],
            timed_out: false,
        },
    );
    let (out, _, code) = brachio(Some(&endpoint), &["task", "wait"]).await;
    assert_eq!(code, 0);
    assert_eq!(out, "c1:agent one\nc2:agent two");
    assert_eq!(
        served.await.expect("app task"),
        MailRequest::WaitTasks {
            key: "task1:agent".into(),
            all: true,
            timeout_secs: 600
        }
    );

    let endpoint = name();
    app(
        &endpoint,
        MailResponse::TasksReported {
            reported: vec![],
            pending: vec![],
            timed_out: false,
        },
    );
    let (out, _, code) = brachio(Some(&endpoint), &["task", "wait"]).await;
    assert_eq!((out.as_str(), code), ("no child tasks", 0));
}
