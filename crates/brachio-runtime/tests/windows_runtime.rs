//! Verifies that separate client processes can reconnect to a Windows shell.

#![cfg(windows)]

use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use uuid::Uuid;

const RUNTIME: &str = env!("CARGO_BIN_EXE_brachio-runtime");

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn server(pipe: &str) -> Server {
    Server(
        Command::new(RUNTIME)
            .args(["--pipe", pipe, "serve"])
            .stdout(Stdio::null())
            .spawn()
            .expect("start runtime"),
    )
}

fn send(pipe: &str, args: &[&str]) -> Output {
    Command::new(RUNTIME)
        .args(["--pipe", pipe, "send"])
        .args(args)
        .output()
        .expect("run client")
}

fn response(output: Output) -> Value {
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("decode response")
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !condition() {
        assert!(Instant::now() < deadline, "condition did not become true");
        thread::sleep(Duration::from_millis(40));
    }
}

#[test]
fn a_new_client_reuses_the_live_shell() {
    let name = format!("brachio-runtime-test-{}", Uuid::new_v4());
    let mut host = server(&name);
    let cwd = std::env::temp_dir();
    let cwd = cwd.to_str().expect("test directory");
    wait_until(|| !send(&name, &["inspect", "missing"]).stdout.is_empty());

    let mut duplicate = server(&name);
    wait_until(|| duplicate.0.try_wait().expect("poll duplicate").is_some());
    assert!(!duplicate.0.wait().expect("join duplicate").success());

    let (first, second) = thread::scope(|scope| {
        let first = scope.spawn(|| response(send(&name, &["ensure", "task", cwd])));
        let second = response(send(&name, &["ensure", "task", cwd]));
        (first.join().expect("join client"), second)
    });
    assert_eq!(first["instance"], second["instance"]);
    assert_eq!(
        first["session"]["session_id"],
        second["session"]["session_id"]
    );
    let pid = first["session"]["pid"].as_u64().expect("shell PID");
    assert_ne!(pid, u64::from(host.0.id()));

    wait_until(|| {
        response(send(&name, &["inspect", "task"]))["session"]["output_bytes"]
            .as_u64()
            .expect("read output count")
            > 4
    });
    response(send(
        &name,
        &[
            "write",
            "task",
            "for /L %i in (1,1,4) do @(ping -n 2 127.0.0.1 >nul & echo brachio-tick-%i)\r",
        ],
    ));
    let before = response(send(&name, &["inspect", "task"]));
    thread::sleep(Duration::from_millis(1600));
    let after = response(send(&name, &["inspect", "task"]));
    assert_eq!(after["session"]["pid"].as_u64(), Some(pid));
    assert_eq!(after["instance"], first["instance"]);
    assert_eq!(
        after["session"]["session_id"],
        first["session"]["session_id"]
    );
    assert!(after["session"]["output_bytes"].as_u64() > before["session"]["output_bytes"].as_u64());

    response(send(&name, &["ensure", "other", cwd]));
    response(send(&name, &["stop", "task"]));
    wait_until(|| !send(&name, &["inspect", "task"]).status.success());
    response(send(&name, &["inspect", "other"]));
    response(send(&name, &["stop", "other"]));
    assert!(host.0.try_wait().expect("poll runtime").is_none());
}
