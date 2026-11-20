//! Runs the real `brachio mcp` against a stand-in for the app's hook socket.

use brachio_runtime::mailbox::{MailRequest, MailResponse};
use brachio_runtime::transport;
use brachio_runtime::wire::{read_message, write_message, MAX_MESSAGE, MAX_RESPONSE};
use serde_json::{json, Value};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

const BRACHIO: &str = env!("CARGO_BIN_EXE_brachio");

/// Answers each connection in turn with the next response and hands back the requests it saw.
fn app(name: &str, responses: Vec<MailResponse>) -> tokio::task::JoinHandle<Vec<MailRequest>> {
    let listener = transport::listen(name).expect("listen");
    tokio::spawn(async move {
        use interprocess::local_socket::tokio::prelude::*;
        let mut seen = Vec::new();
        for response in responses {
            let stream = listener.accept().await.expect("accept");
            let mut reader = BufReader::new(stream);
            let message = read_message(&mut reader, MAX_MESSAGE)
                .await
                .expect("read")
                .expect("a request");
            seen.push(serde_json::from_slice(&message).expect("decode"));
            write_message(reader.get_mut(), &response, MAX_RESPONSE)
                .await
                .expect("answer");
            let _ = reader.get_mut().shutdown().await;
        }
        seen
    })
}

fn name() -> String {
    format!("mcp-{}", &Uuid::new_v4().simple().to_string()[..12])
}

/// Sends the handshake and `calls` to `brachio mcp`, and returns the replies after it.
async fn mcp(endpoint: Option<&str>, calls: &[Value]) -> Vec<Value> {
    let mut input = String::new();
    for line in [
        json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ]
    .iter()
    .chain(calls)
    {
        input.push_str(&format!("{line}\n"));
    }
    let mut command = Command::new(BRACHIO);
    command
        .arg("mcp")
        .env_remove("BRACHIO_HOOK_ENDPOINT")
        .env_remove("BRACHIO_TERMINAL_KEY")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    if let Some(endpoint) = endpoint {
        command
            .env("BRACHIO_HOOK_ENDPOINT", endpoint)
            .env("BRACHIO_TERMINAL_KEY", "task1:agent");
    }
    let mut child = command.spawn().expect("run brachio mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    stdin.write_all(input.as_bytes()).await.expect("write");
    drop(stdin);
    let output = child.wait_with_output().await.expect("finish");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .map(|line| serde_json::from_str(line).expect("JSON reply"))
        .collect()
}

fn tool(id: u32, name: &str, arguments: &Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})
}

#[tokio::test]
async fn spawn_task_asks_the_app_with_this_terminals_identity_and_returns_the_new_key() {
    let endpoint = name();
    let served = app(
        &endpoint,
        vec![MailResponse::TaskCreated {
            key: "task2:agent".into(),
            title: "Docs".into(),
            branch: "brachio/docs".into(),
        }],
    );
    let replies = mcp(
        Some(&endpoint),
        &[tool(
            1,
            "spawn_task",
            &json!({"title":"Docs","prompt":"Write","agent":"codex"}),
        )],
    )
    .await;
    assert!(replies[0]["result"].get("isError").is_none(), "{replies:?}");
    let text = replies[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    assert_eq!(
        serde_json::from_str::<Value>(text).expect("JSON"),
        json!({"key":"task2:agent","branch":"brachio/docs"})
    );
    assert_eq!(
        served.await.expect("app"),
        [MailRequest::CreateTask {
            key: "task1:agent".into(),
            title: "Docs".into(),
            prompt: "Write".into(),
            agent: Some("codex".into()),
            base: None,
            after: Vec::new(),
        }]
    );
}

#[tokio::test]
async fn a_refused_approval_comes_back_as_an_mcp_tool_error() {
    let endpoint = name();
    let served = app(
        &endpoint,
        vec![MailResponse::Error {
            message: "The user did not allow this task.".into(),
        }],
    );
    let replies = mcp(
        Some(&endpoint),
        &[tool(
            1,
            "spawn_task",
            &json!({"title":"Docs","prompt":"Write"}),
        )],
    )
    .await;
    assert_eq!(replies[0]["result"]["isError"], true);
    assert_eq!(
        replies[0]["result"]["content"][0]["text"],
        "The user did not allow this task."
    );
    served.await.expect("app");
}

#[tokio::test]
async fn oversize_input_is_refused_before_anything_reaches_the_app() {
    let endpoint = name();
    let served = app(&endpoint, vec![]);
    let replies = mcp(
        Some(&endpoint),
        &[
            tool(
                1,
                "spawn_task",
                &json!({"title":"Docs","prompt":"x".repeat(9 * 1024)}),
            ),
            tool(2, "request_review", &json!({"subject":"s".repeat(201)})),
        ],
    )
    .await;
    assert_eq!(replies[0]["error"]["code"], -32602);
    assert_eq!(replies[1]["error"]["code"], -32602);
    assert!(served.await.expect("app").is_empty());
}

#[tokio::test]
async fn request_review_sends_a_merge_ready_mail_to_the_lead() {
    let endpoint = name();
    let served = app(
        &endpoint,
        vec![MailResponse::Sent {
            id: "m1".into(),
            thread_id: "m1".into(),
            to: "lead:agent".into(),
            duplicate: false,
        }],
    );
    let replies = mcp(
        Some(&endpoint),
        &[tool(
            1,
            "request_review",
            &json!({"subject":"Ready","body":"See diff"}),
        )],
    )
    .await;
    assert!(replies[0]["result"].get("isError").is_none(), "{replies:?}");
    let requests = served.await.expect("app");
    let MailRequest::Send(mail) = &requests[0] else {
        panic!("expected a send: {requests:?}");
    };
    assert_eq!(
        (
            mail.from.as_str(),
            mail.to.as_str(),
            mail.kind.as_str(),
            mail.subject.as_str(),
            mail.body.as_str()
        ),
        ("task1:agent", "parent", "merge_ready", "Ready", "See diff")
    );
}

#[tokio::test]
async fn get_task_result_waits_for_a_report_then_lists_the_children() {
    let endpoint = name();
    let served = app(
        &endpoint,
        vec![
            MailResponse::TasksReported {
                reported: vec![],
                pending: vec!["task2:agent".into()],
                timed_out: true,
            },
            MailResponse::TaskList { children: vec![] },
        ],
    );
    let replies = mcp(
        Some(&endpoint),
        &[tool(1, "get_task_result", &json!({"wait_secs":1}))],
    )
    .await;
    let text = replies[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    assert_eq!(
        serde_json::from_str::<Value>(text).expect("JSON"),
        json!({"children":[],"wait":{"reported":[],"pending":["task2:agent"],"timed_out":true}})
    );
    let requests = served.await.expect("app");
    assert_eq!(
        requests[0],
        MailRequest::WaitTasks {
            key: "task1:agent".into(),
            all: false,
            timeout_secs: 1
        }
    );
}

#[tokio::test]
async fn a_closed_or_missing_socket_is_a_tool_error_not_a_crash() {
    let replies = mcp(
        Some(&name()),
        &[
            tool(1, "get_task_result", &json!({})),
            tool(2, "request_review", &json!({"subject":"s"})),
        ],
    )
    .await;
    assert_eq!(replies[0]["result"]["isError"], true);
    assert_eq!(replies[1]["result"]["isError"], true);
    let outside = mcp(
        None,
        &[tool(1, "spawn_task", &json!({"title":"t","prompt":"p"}))],
    )
    .await;
    assert_eq!(outside[0]["result"]["isError"], true);
}
