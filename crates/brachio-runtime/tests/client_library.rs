//! Verifies the client library the app uses against a real runtime process.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use brachio_runtime::client::{Client, ClientError};
use brachio_runtime::wire::{Action, ResponseResult};
use uuid::Uuid;

const RUNTIME: &str = env!("CARGO_BIN_EXE_brachio-runtime");

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn endpoint() -> String {
    format!("client-{}", &Uuid::new_v4().simple().to_string()[..12])
}

fn server(name: &str) -> Server {
    let state = std::env::temp_dir().join(format!("brachio-state-{name}"));
    Server(
        Command::new(RUNTIME)
            .args(["--pipe", name, "--state-dir"])
            .arg(state)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start runtime"),
    )
}

async fn ready(client: &Client) {
    for _ in 0..200 {
        if client.call(Action::Hello).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    panic!("runtime did not become ready");
}

#[tokio::test]
async fn a_missing_runtime_is_reported_as_not_running() {
    let client = Client::new(&endpoint());
    let error = client.call(Action::Hello).await.expect_err("no runtime");
    assert!(matches!(error, ClientError::NotRunning), "{error:?}");
}

#[tokio::test]
async fn the_app_can_start_follow_and_write_to_a_shell() {
    let name = endpoint();
    let _host = server(&name);
    let client = Client::new(&name);
    ready(&client).await;

    let cwd = std::env::temp_dir();
    client
        .call(Action::Ensure {
            key: "task".to_owned(),
            cwd,
        })
        .await
        .expect("start shell");
    let refused = client
        .call(Action::Inspect {
            key: "missing".to_owned(),
        })
        .await
        .expect_err("unknown shell");
    assert!(matches!(refused, ClientError::Refused(_)), "{refused:?}");

    let mut offset = None;
    for _ in 0..200 {
        let (first, _) = match client
            .follow(Action::FollowFrame {
                key: "task".to_owned(),
            })
            .await
        {
            Ok(opened) => opened,
            Err(ClientError::Refused(_)) => {
                tokio::time::sleep(Duration::from_millis(40)).await;
                continue;
            }
            Err(error) => panic!("{error}"),
        };
        if let ResponseResult::Snapshot { screen, .. } = first.result {
            offset = screen.resume_offset;
            break;
        }
    }
    assert!(offset.is_some(), "no settled frame to follow from");

    let (_, mut following) = client
        .follow(Action::Follow {
            key: "task".to_owned(),
            after: offset.expect("offset"),
        })
        .await
        .expect("follow output");
    let ending = if cfg!(windows) { "\r" } else { "\n" };
    client
        .call(Action::Write {
            key: "task".to_owned(),
            text: format!("echo brachio-client-ok{ending}"),
        })
        .await
        .expect("write");

    let mut seen = Vec::new();
    while !String::from_utf8_lossy(&seen).contains("brachio-client-ok") {
        let reply = tokio::time::timeout(Duration::from_secs(8), following.next())
            .await
            .expect("output arrived in time")
            .expect("follow reply")
            .expect("connection open");
        if let ResponseResult::Success {
            output: Some(page), ..
        } = reply.result
        {
            seen.extend(page.bytes);
        }
    }
    client
        .call(Action::Stop {
            key: "task".to_owned(),
        })
        .await
        .expect("stop");
}
