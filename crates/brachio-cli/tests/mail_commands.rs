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
async fn check_prints_one_line_per_message_and_what_is_left() {
    let endpoint = name();
    let served = app(
        &endpoint,
        MailResponse::Messages {
            messages: vec![MailMessage {
                id: "m9".into(),
                thread_id: "m9".into(),
                from: "task2:agent".into(),
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
    assert!(out.contains("dev server"));
    assert!(out.ends_with("2 more unread; run brachio check again"));
    assert!(matches!(
        served.await.expect("app task"),
        MailRequest::Check { peek: false, .. }
    ));
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
