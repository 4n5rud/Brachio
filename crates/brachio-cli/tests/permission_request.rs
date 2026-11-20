//! Runs the real `brachio` binary the way an agent's hook would, against a stand-in for the app.

use std::process::Stdio;
use std::time::Duration;

use brachio_runtime::approval::{ApprovalRequest, Decision};
use brachio_runtime::transport;
use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

const BRACHIO: &str = env!("CARGO_BIN_EXE_brachio");
const QUESTION: &str = r#"{"session_id":"s1","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"pnpm add zod"},"tool_use_id":"t1"}"#;

enum Reply {
    Say(Decision),
    Close,
    Stall,
}

fn endpoint() -> String {
    format!("hook-{}", &Uuid::new_v4().simple().to_string()[..12])
}

/// Accepts one question, returns what the hook asked, and answers as told.
fn app(name: &str, reply: Reply) -> tokio::task::JoinHandle<ApprovalRequest> {
    let listener = transport::listen(name).expect("listen");
    tokio::spawn(async move {
        use interprocess::local_socket::tokio::prelude::*;
        let stream = listener.accept().await.expect("accept");
        let mut reader = BufReader::new(stream);
        let message = read_message(&mut reader, MAX_MESSAGE)
            .await
            .expect("read")
            .expect("a question");
        let asked: ApprovalRequest = serde_json::from_slice(&message).expect("decode");
        match reply {
            Reply::Say(decision) => {
                write_message(reader.get_mut(), &decision, MAX_RESPONSE)
                    .await
                    .expect("answer");
            }
            Reply::Close => {}
            Reply::Stall => tokio::time::sleep(Duration::from_secs(30)).await,
        }
        let _ = reader.get_mut().shutdown().await;
        asked
    })
}

/// Runs the hook and returns its standard output and exit status.
async fn hook(name: Option<&str>, input: &str, wait_seconds: &str) -> (String, i32) {
    let mut command = Command::new(BRACHIO);
    command
        .args(["hook", "permission-request"])
        .env("BRACHIO_APPROVAL_WAIT_SECONDS", wait_seconds)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
        .env_remove("BRACHIO_HOOK_ENDPOINT")
        .env_remove("BRACHIO_TERMINAL_KEY");
    if let Some(name) = name {
        command
            .env("BRACHIO_HOOK_ENDPOINT", name)
            .env("BRACHIO_TERMINAL_KEY", "task1:agent");
    }
    let mut child = command.spawn().expect("start brachio");
    let mut stdin = child.stdin.take().expect("stdin");
    stdin
        .write_all(input.as_bytes())
        .await
        .expect("send question");
    drop(stdin);
    let output = child.wait_with_output().await.expect("finish");
    (
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[tokio::test]
async fn an_approval_in_the_app_becomes_the_agents_allow_answer() {
    let name = endpoint();
    let server = app(&name, Reply::Say(Decision::Allow));
    let (output, status) = hook(Some(&name), QUESTION, "10").await;
    let asked = server.await.expect("app task");
    assert_eq!(status, 0);
    assert_eq!(asked.key, "task1:agent");
    assert_eq!(
        asked.review().as_deref(),
        Some("\"Bash\"\n{\"command\":\"pnpm add zod\"}")
    );
    let answer: serde_json::Value = serde_json::from_str(&output).expect("json answer");
    assert_eq!(
        answer["hookSpecificOutput"]["decision"]["behavior"],
        "allow"
    );
}

#[tokio::test]
async fn an_oversized_request_falls_back_even_when_the_app_would_allow_it() {
    let name = endpoint();
    let server = app(&name, Reply::Say(Decision::Allow));
    let content = "x".repeat(3_000);
    let input = format!(
        r#"{{"session_id":"s","tool_name":"Write","tool_input":{{"file_path":"example","content":"{content}"}}}}"#
    );
    let (output, status) = hook(Some(&name), &input, "10").await;
    assert_eq!((output.as_str(), status), ("", 0));
    server.abort();
}

#[tokio::test]
async fn a_denial_in_the_app_becomes_the_agents_deny_answer() {
    let name = endpoint();
    let server = app(
        &name,
        Reply::Say(Decision::Deny {
            message: "Denied in Brachio".to_owned(),
        }),
    );
    let (output, status) = hook(Some(&name), QUESTION, "10").await;
    server.await.expect("app task");
    assert_eq!(status, 0);
    let answer: serde_json::Value = serde_json::from_str(&output).expect("json answer");
    assert_eq!(answer["hookSpecificOutput"]["decision"]["behavior"], "deny");
}

#[tokio::test]
async fn every_failure_prints_nothing_and_exits_normally_so_the_agent_asks_in_the_terminal() {
    // The app says "ask", closes the connection, or never answers.
    for (reply, wait) in [
        (Reply::Say(Decision::Ask), "10"),
        (Reply::Close, "10"),
        (Reply::Stall, "1"),
    ] {
        let name = endpoint();
        let server = app(&name, reply);
        let (output, status) = hook(Some(&name), QUESTION, wait).await;
        server.abort();
        assert_eq!((output.as_str(), status), ("", 0));
    }
    // The app is not running.
    let (output, status) = hook(Some(&endpoint()), QUESTION, "10").await;
    assert_eq!((output.as_str(), status), ("", 0));
    // The hook was not started by Brachio, or was given something else on standard input.
    let (output, status) = hook(None, QUESTION, "10").await;
    assert_eq!((output.as_str(), status), ("", 0));
    let (output, status) = hook(Some(&endpoint()), "not json", "10").await;
    assert_eq!((output.as_str(), status), ("", 0));
}
