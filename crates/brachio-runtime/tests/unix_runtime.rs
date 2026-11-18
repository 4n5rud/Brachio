//! Verifies the runtime over a Unix socket: reuse, ownership and stale endpoints.

#![cfg(unix)]

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

fn endpoint() -> String {
    // Unix socket paths are short, so keep the name short too.
    format!("test-{}", &Uuid::new_v4().simple().to_string()[..12])
}

fn server(name: &str) -> Server {
    Server(
        Command::new(RUNTIME)
            .args(["--pipe", name, "serve"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start runtime"),
    )
}

fn send(name: &str, args: &[&str]) -> Output {
    Command::new(RUNTIME)
        .args(["--pipe", name, "send"])
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

fn ready(name: &str) {
    wait_until(|| !send(name, &["hello"]).stdout.is_empty());
}

#[test]
fn a_new_client_reuses_the_live_shell_and_a_second_runtime_is_refused() {
    let name = endpoint();
    let host = server(&name);
    ready(&name);
    let cwd = std::env::temp_dir();
    let cwd = cwd.to_str().expect("test directory");

    let first = response(send(&name, &["ensure", "task", cwd]));
    let second = response(send(&name, &["ensure", "task", cwd]));
    assert_eq!(first["instance"], second["instance"]);
    assert_eq!(first["session"]["pid"], second["session"]["pid"]);
    assert_ne!(
        first["session"]["pid"].as_u64(),
        Some(u64::from(host.0.id()))
    );

    response(send(&name, &["write", "task", "echo brachio-unix-ok\n"]));
    wait_until(|| {
        let page = response(send(&name, &["read", "task", "0"]));
        let bytes = page["output"]["bytes"].as_array().expect("output bytes");
        let text: Vec<u8> = bytes
            .iter()
            .filter_map(|b| b.as_u64())
            .map(|b| b as u8)
            .collect();
        String::from_utf8_lossy(&text).contains("brachio-unix-ok")
    });

    let mut duplicate = server(&name);
    assert_eq!(duplicate.0.wait().expect("join duplicate").code(), Some(75));
    response(send(&name, &["inspect", "task"]));
}

#[test]
fn a_leftover_socket_of_a_killed_runtime_is_replaced() {
    let name = endpoint();
    let mut first = server(&name);
    ready(&name);
    first.0.kill().expect("kill runtime");
    first.0.wait().expect("reap runtime");

    let _second = server(&name);
    ready(&name);
    assert_eq!(
        response(send(&name, &["hello"]))["runtime"]["current_version"],
        2
    );
}

#[test]
fn a_draining_runtime_exits_after_its_last_shell() {
    let name = endpoint();
    let mut host = server(&name);
    ready(&name);
    let cwd = std::env::temp_dir();
    let cwd = cwd.to_str().expect("test directory");
    response(send(&name, &["ensure", "kept", cwd]));
    response(send(&name, &["drain"]));
    assert!(host.0.try_wait().expect("poll runtime").is_none());
    response(send(&name, &["stop", "kept"]));
    wait_until(|| host.0.try_wait().expect("poll runtime").is_some());
}
