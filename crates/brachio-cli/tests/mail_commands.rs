//! Runs the real `brachio send` and `brachio check` against a stand-in for the app.

use brachio_runtime::mailbox::{MailMessage, MailRequest, MailResponse, MAX_BODY_BYTES};
use brachio_runtime::transport;
use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

const BRACHIO: &str = env!("CARGO_BIN_EXE_brachio");

/// Accepts one request, hands it back and answers with `response`.
fn app(name: &str, response: MailResponse) -> tokio::task::JoinHandle<MailRequest> {
    let listener = transport::listen(name).expect("listen");
    tokio::spawn(async move { serve_one(&listener, &response).await })
}

/// Accepts one connection per response, in order, and returns the requests it saw.
fn app_in_order(
    name: &str,
    responses: Vec<MailResponse>,
) -> tokio::task::JoinHandle<Vec<MailRequest>> {
    let listener = transport::listen(name).expect("listen");
    tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in &responses {
            requests.push(serve_one(&listener, response).await);
        }
        requests
    })
}

async fn serve_one(
    listener: &interprocess::local_socket::tokio::Listener,
    response: &MailResponse,
) -> MailRequest {
    use interprocess::local_socket::tokio::prelude::*;
    let stream = listener.accept().await.expect("accept");
    let mut reader = BufReader::new(stream);
    let message = read_message(&mut reader, MAX_MESSAGE)
        .await
        .expect("read")
        .expect("a request");
    let request: MailRequest = serde_json::from_slice(&message).expect("decode");
    write_message(reader.get_mut(), response, MAX_RESPONSE)
        .await
        .expect("answer");
    if let (MailRequest::Check { peek: false, .. }, MailResponse::Messages { messages, .. }) =
        (&request, response)
    {
        if !messages.is_empty() {
            let ack = read_message(&mut reader, MAX_MESSAGE)
                .await
                .expect("read ack")
                .expect("an acknowledgement");
            let ids = messages.iter().map(|message| message.id.clone()).collect();
            assert_eq!(
                serde_json::from_slice::<MailRequest>(&ack).expect("decode ack"),
                MailRequest::Ack { ids }
            );
        }
    }
    let _ = reader.get_mut().shutdown().await;
    request
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

/// Runs `brachio send --body -` with `body` on standard input; returns stderr and the exit code.
async fn send_from_stdin(endpoint: &str, body: Vec<u8>) -> (String, i32) {
    use tokio::io::AsyncWriteExt as _;
    let mut child = Command::new(BRACHIO)
        .args([
            "send",
            "--to",
            "Write docs",
            "--subject",
            "Log",
            "--body",
            "-",
        ])
        .env("BRACHIO_HOOK_ENDPOINT", endpoint)
        .env("BRACHIO_TERMINAL_KEY", "task1:agent")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start brachio");
    let mut stdin = child.stdin.take().expect("stdin");
    let writer = tokio::spawn(async move {
        // The command may stop reading early, so a failed write is expected here.
        let _ = stdin.write_all(&body).await;
    });
    let output = child.wait_with_output().await.expect("finish");
    writer.await.expect("writer task");
    (
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[tokio::test]
async fn send_accepts_a_body_from_standard_input_at_exactly_the_body_limit() {
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
    let (error, code) = send_from_stdin(&endpoint, vec![b'x'; MAX_BODY_BYTES]).await;
    assert_eq!(code, 0, "{error}");
    let MailRequest::Send(sent) = served.await.expect("app task") else {
        panic!("expected a send request");
    };
    assert_eq!(sent.body.len(), MAX_BODY_BYTES);
}

#[tokio::test]
async fn send_refuses_a_body_from_standard_input_over_the_body_limit_before_connecting() {
    let expected = format!("larger than {MAX_BODY_BYTES} bytes");
    // No app listens on this endpoint: a refusal naming the size proves nothing was sent.
    for size in [MAX_BODY_BYTES + 1, MAX_BODY_BYTES * 128] {
        let (error, code) = send_from_stdin(&name(), vec![b'x'; size]).await;
        assert_ne!(code, 0);
        assert!(error.contains(&expected), "{size} bytes: {error}");
    }
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
            after: Vec::new(),
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

#[tokio::test]
async fn mcp_lists_child_status_using_the_cli_terminal_identity() {
    let endpoint = name();
    let request = app(&endpoint, MailResponse::TaskList { children: vec![] });
    let mut process = Command::new(BRACHIO)
        .arg("mcp")
        .env("BRACHIO_HOOK_ENDPOINT", &endpoint)
        .env("BRACHIO_TERMINAL_KEY", "task1:agent")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("start MCP server");
    let mut input = process.stdin.take().expect("stdin");
    let requests = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_child_tasks"}}"#,
        "\n",
    );
    input
        .write_all(requests.as_bytes())
        .await
        .expect("send MCP requests");
    drop(input);
    let output = process.wait_with_output().await.expect("MCP response");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 protocol");
    let last: serde_json::Value =
        serde_json::from_str(stdout.lines().last().expect("tool response")).expect("JSON response");
    assert_eq!(last["result"]["content"][0]["text"], "[]");
    let observed = request.await.expect("receive socket request");
    assert!(matches!(observed, MailRequest::ListTasks { key } if key == "task1:agent"));
}

fn waiting_message(id: &str) -> MailMessage {
    MailMessage {
        id: id.into(),
        thread_id: id.into(),
        from: "task2:agent".into(),
        to: "task1:agent".into(),
        from_title: Some("Write docs".into()),
        kind: "status".into(),
        priority: "normal".into(),
        subject: "Ready".into(),
        body: String::new(),
        payload: None,
        created_at_ms: 0,
        read: false,
    }
}

#[tokio::test]
async fn check_wait_looks_without_taking_then_takes_and_acknowledges_what_arrived() {
    let endpoint = name();
    let messages = |unread_left| MailResponse::Messages {
        messages: vec![waiting_message("m5")],
        unread_left,
    };
    let served = app_in_order(&endpoint, vec![messages(1), messages(0)]);
    let (out, err, code) =
        brachio(Some(&endpoint), &["check", "--wait", "--timeout-secs", "5"]).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "m5 [status] from Write docs: Ready");
    let requests = served.await.expect("app task");
    assert!(matches!(requests[0], MailRequest::Check { peek: true, .. }));
    assert!(matches!(
        requests[1],
        MailRequest::Check { peek: false, .. }
    ));
}

#[tokio::test]
async fn check_peek_takes_nothing_and_sends_no_acknowledgement() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::Messages {
            messages: vec![waiting_message("m6")],
            unread_left: 0,
        },
    );
    let (out, _, code) = brachio(Some(&endpoint), &["check", "--peek"]).await;
    assert_eq!(code, 0);
    assert_eq!(out, "m6 [status] from Write docs: Ready");
    assert!(matches!(
        served.await.expect("app task"),
        MailRequest::Check { peek: true, .. }
    ));
}

#[tokio::test]
async fn inbox_lists_recent_mail_with_its_limit() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::Messages {
            messages: vec![waiting_message("m7")],
            unread_left: 0,
        },
    );
    let (out, _, code) = brachio(Some(&endpoint), &["inbox", "--limit", "5"]).await;
    assert_eq!(code, 0);
    assert_eq!(out, "m7 [status] from Write docs: Ready");
    assert_eq!(
        served.await.expect("app task"),
        MailRequest::Inbox {
            key: "task1:agent".into(),
            limit: Some(5)
        }
    );
}

#[tokio::test]
async fn peers_prints_key_title_and_branch_or_says_there_are_none() {
    use brachio_runtime::mailbox::Peer;

    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::Peers {
            peers: vec![Peer {
                key: "task2:agent".into(),
                title: "Write docs".into(),
                branch: "brachio/write-docs".into(),
            }],
        },
    );
    let (out, _, code) = brachio(Some(&endpoint), &["peers"]).await;
    assert_eq!(code, 0);
    assert_eq!(out, "task2:agent  Write docs  (brachio/write-docs)");
    assert_eq!(
        served.await.expect("app task"),
        MailRequest::Peers {
            key: "task1:agent".into()
        }
    );

    let endpoint = name();
    app(&endpoint, MailResponse::Peers { peers: vec![] });
    let (out, _, _) = brachio(Some(&endpoint), &["peers"]).await;
    assert_eq!(out, "no other open tasks");
}

#[tokio::test]
async fn send_to_parent_with_worker_done_reaches_the_app_unresolved() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::Sent {
            id: "m8".into(),
            thread_id: "m8".into(),
            to: "lead:agent".into(),
            duplicate: false,
        },
    );
    let (out, _, code) = brachio(
        Some(&endpoint),
        &[
            "send",
            "--to",
            "parent",
            "--type",
            "worker_done",
            "--subject",
            "done",
        ],
    )
    .await;
    assert_eq!((out.as_str(), code), ("sent m8 to lead:agent", 0));
    let MailRequest::Send(sent) = served.await.expect("app task") else {
        panic!("expected a send request");
    };
    assert_eq!(
        (sent.to.as_str(), sent.kind.as_str()),
        ("parent", "worker_done")
    );
}

#[tokio::test]
async fn task_handoff_sends_the_summary_and_tests_and_prints_the_short_commit() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::HandedOff {
            commit_sha: "0123456789abcdef0123".into(),
            changed_paths: 3,
        },
    );
    let (out, err, code) = brachio(
        Some(&endpoint),
        &[
            "task",
            "handoff",
            "--summary",
            "Added parser",
            "--tests",
            r#"["cargo test"]"#,
        ],
    )
    .await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "handed off 0123456789ab (3 changed files); waiting for the user's review"
    );
    assert_eq!(
        served.await.expect("app task"),
        MailRequest::Handoff {
            key: "task1:agent".into(),
            summary: "Added parser".into(),
            tests: vec!["cargo test".into()]
        }
    );
}
