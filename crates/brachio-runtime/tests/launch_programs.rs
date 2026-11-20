//! Verifies that the runtime can host arbitrary programs and report what they say about themselves.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use brachio_runtime::client::Client;
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

fn start() -> (Server, Client) {
    let name = format!("launch-{}", &Uuid::new_v4().simple().to_string()[..12]);
    let state = std::env::temp_dir().join(format!("brachio-state-{name}"));
    let server = Server(
        Command::new(RUNTIME)
            .args(["--pipe", &name, "--state-dir"])
            .arg(state)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start runtime"),
    );
    (server, Client::new(&name))
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

fn launch(program: &str, args: &[&str], env: &[&str]) -> Action {
    Action::Launch {
        key: "agent".to_owned(),
        cwd: std::env::temp_dir(),
        program: program.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        env: env.iter().map(|entry| (*entry).to_owned()).collect(),
        rows: 30,
        cols: 100,
    }
}

#[tokio::test]
async fn a_launched_program_receives_its_arguments_and_environment() {
    let (_host, client) = start();
    ready(&client).await;
    let action = if cfg!(windows) {
        launch(
            "cmd.exe",
            &["/d", "/c", "echo %BRACHIO_PROBE%"],
            &["BRACHIO_PROBE=launched-ok"],
        )
    } else {
        launch(
            "sh",
            &["-c", "echo $BRACHIO_PROBE"],
            &["BRACHIO_PROBE=launched-ok"],
        )
    };
    client.call(action).await.expect("launch");

    let (_, mut following) = client
        .follow(Action::Follow {
            key: "agent".to_owned(),
            after: 0,
        })
        .await
        .expect("follow");
    let mut seen = Vec::new();
    loop {
        let reply = tokio::time::timeout(Duration::from_secs(8), following.next())
            .await
            .expect("output arrived in time")
            .expect("follow reply");
        let Some(reply) = reply else { break };
        if let ResponseResult::Success { session, output } = reply.result {
            if let Some(page) = output {
                seen.extend(page.bytes);
            }
            if session.exited {
                break;
            }
        }
    }
    assert!(String::from_utf8_lossy(&seen).contains("launched-ok"));
}

#[tokio::test]
async fn the_first_snapshot_uses_the_dimensions_requested_at_launch() {
    let (_host, client) = start();
    ready(&client).await;
    let (program, args): (&str, &[&str]) = if cfg!(windows) {
        ("cmd.exe", &["/d"])
    } else {
        ("sh", &["-i"])
    };
    client
        .call(launch(program, args, &[]))
        .await
        .expect("launch");
    let reply = client
        .call(Action::Snapshot {
            key: "agent".to_owned(),
        })
        .await
        .expect("read initial snapshot");
    client
        .call(Action::Stop {
            key: "agent".to_owned(),
        })
        .await
        .expect("stop test shell");
    let ResponseResult::Snapshot { screen, .. } = reply.result else {
        panic!("expected a screen snapshot");
    };
    assert_eq!((screen.rows, screen.cols), (30, 100));
}

#[tokio::test]
async fn a_malformed_environment_entry_is_refused() {
    let (_host, client) = start();
    ready(&client).await;
    let program = if cfg!(windows) { "cmd.exe" } else { "sh" };
    let error = client
        .call(launch(program, &[], &["NO_EQUALS_SIGN"]))
        .await
        .expect_err("malformed environment");
    assert!(error.to_string().contains("NAME=value"), "{error}");
}

#[cfg(unix)]
#[tokio::test]
async fn status_reports_from_the_program_reach_followers() {
    let (_host, client) = start();
    ready(&client).await;
    let script = "printf '\\033]7501;state=working\\007'; sleep 1; printf '\\033]7501;state=idle\\007'; sleep 3";
    client
        .call(launch("sh", &["-c", script], &[]))
        .await
        .expect("launch");

    let (_, mut following) = client
        .follow(Action::Follow {
            key: "agent".to_owned(),
            after: 0,
        })
        .await
        .expect("follow");
    let mut states = Vec::new();
    while !states.contains(&"idle") {
        let reply = tokio::time::timeout(Duration::from_secs(8), following.next())
            .await
            .expect("status arrived in time")
            .expect("follow reply")
            .expect("connection open");
        if let ResponseResult::Success { session, .. } = reply.result {
            let state = session
                .status
                .map(|status| serde_json::to_value(status).expect("encode status"))
                .and_then(|value| value["state"].as_str().map(str::to_owned));
            match state.as_deref() {
                Some("working") if !states.contains(&"working") => states.push("working"),
                Some("idle") => states.push("idle"),
                _ => {}
            }
        }
    }
    assert_eq!(states, ["working", "idle"]);
}
