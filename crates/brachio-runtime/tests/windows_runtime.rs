//! Verifies that separate client processes can reconnect to a Windows shell.

#![cfg(windows)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
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
    let before_offset = before["session"]["output_bytes"].to_string();
    let restored = response(send(&name, &["read", "task", &before_offset]));
    let page = &restored["output"];
    assert_eq!(page["start"], before["session"]["output_bytes"]);
    assert_eq!(page["truncated"], false);
    let bytes: Vec<u8> = serde_json::from_value(page["bytes"].clone()).expect("read output bytes");
    assert!(
        String::from_utf8_lossy(&bytes).contains("brachio-tick-"),
        "{bytes:?}"
    );
    let next = page["next"].as_u64().expect("next byte offset");
    assert_eq!(
        next - page["start"].as_u64().expect("first offset"),
        bytes.len() as u64
    );
    let following = response(send(&name, &["read", "task", &next.to_string()]));
    assert_eq!(following["output"]["start"], next);

    let mut follower = Server(
        Command::new(RUNTIME)
            .args(["--pipe", &name, "send", "follow", "task", &before_offset])
            .stdout(Stdio::piped())
            .spawn()
            .expect("start follower"),
    );
    let stdout = follower.0.stdout.take().expect("follower output");
    let (frames, received) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if frames
                .send(serde_json::from_str::<Value>(&line).expect("decode frame"))
                .is_err()
            {
                break;
            }
        }
    });
    let frame = || {
        received
            .recv_timeout(Duration::from_secs(8))
            .expect("receive output frame")
    };
    let replayed = frame();
    assert_eq!(
        replayed["output"]["start"],
        before["session"]["output_bytes"]
    );
    let mut offset = replayed["output"]["next"]
        .as_u64()
        .expect("replayed offset");
    response(send(
        &name,
        &["write", "task", "echo brachio-live-marker\r"],
    ));
    let mut live = Vec::new();
    while !String::from_utf8_lossy(&live).contains("brachio-live-marker") {
        let current = frame();
        assert_eq!(current["output"]["start"], offset);
        offset = current["output"]["next"]
            .as_u64()
            .expect("following offset");
        live.extend(
            serde_json::from_value::<Vec<u8>>(current["output"]["bytes"].clone())
                .expect("decode bytes"),
        );
    }

    response(send(&name, &["ensure", "other", cwd]));
    response(send(&name, &["stop", "task"]));
    loop {
        let current = frame();
        assert_eq!(current["output"]["start"], offset);
        offset = current["output"]["next"].as_u64().expect("final offset");
        if current["session"]["exited"] == true {
            break;
        }
    }
    wait_until(|| follower.0.try_wait().expect("poll follower").is_some());
    assert!(follower.0.wait().expect("join follower").success());
    reader.join().expect("join follower output reader");
    wait_until(|| !send(&name, &["inspect", "task"]).status.success());
    response(send(&name, &["inspect", "other"]));
    response(send(&name, &["stop", "other"]));
    assert!(host.0.try_wait().expect("poll runtime").is_none());
}

#[test]
fn a_slow_follower_does_not_block_other_clients() {
    let name = format!("brachio-runtime-test-{}", Uuid::new_v4());
    let _host = server(&name);
    let cwd = std::env::temp_dir();
    let cwd = cwd.to_str().expect("test directory");
    wait_until(|| !send(&name, &["inspect", "missing"]).stdout.is_empty());
    response(send(&name, &["ensure", "burst", cwd]));
    wait_until(|| {
        response(send(&name, &["inspect", "burst"]))["session"]["output_bytes"]
            .as_u64()
            .expect("read output count")
            > 4
    });
    let _slow = Server(
        Command::new(RUNTIME)
            .args(["--pipe", &name, "send", "follow", "burst", "0"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("start unread follower"),
    );
    let line = "x".repeat(64);
    let command = format!("for /L %i in (1,1,5000) do @echo {line}\r");
    response(send(&name, &["write", "burst", &command]));
    wait_until(|| {
        response(send(&name, &["inspect", "burst"]))["session"]["output_bytes"]
            .as_u64()
            .expect("read output count")
            > 256 * 1024
    });
    let restored = response(send(&name, &["read", "burst", "0"]));
    assert_eq!(restored["output"]["truncated"], true);
    assert!(
        restored["output"]["start"]
            .as_u64()
            .expect("retained start")
            > 0
    );
    response(send(
        &name,
        &["write", "burst", "cls & echo brachio-frame-marker\r"],
    ));
    wait_until(|| {
        let frame = response(send(&name, &["snapshot", "burst"]));
        let ansi = frame["screen"]["ansi"]
            .as_str()
            .expect("visible screen ANSI");
        ansi.contains("brachio-frame-marker") && !ansi.contains("xxxxxxxxxx")
    });
    response(send(&name, &["stop", "burst"]));
}
