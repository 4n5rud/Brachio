//! Opt-in check that memory stays under a documented bound as shell sessions are added.
//!
//! It starts a real runtime with 1, 5 and 10 shell sessions that have each printed enough to
//! fill part of their output history, reads every session the way the app does, and compares the
//! working set of the runtime's process tree and of this process with the bounds in
//! `docs/windows-memory-benchmark.md`. It is ignored by default; run it with
//! `cargo test -p brachio-runtime --test memory_bound -- --ignored --nocapture`.
//!
//! This process stands in for the app side: it holds the client library and the output pages the
//! app reads, but not the web view, so it is not the figure the status bar shows.

#![cfg(windows)]

use std::collections::HashSet;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use brachio_runtime::client::Client;
use brachio_runtime::wire::{Action, ResponseResult};
use sysinfo::{Pid, ProcessesToUpdate, System};
use uuid::Uuid;

const RUNTIME: &str = env!("CARGO_BIN_EXE_brachio-runtime");
const MIB: u64 = 1024 * 1024;

/// Working set of the runtime and its shells: a fixed part plus a part for each session.
const RUNTIME_BASE_MIB: u64 = 48;
const RUNTIME_PER_SESSION_MIB: u64 = 24;
/// Working set of this client process: a fixed part plus a part for each session it read.
const APP_SIDE_BASE_MIB: u64 = 32;
const APP_SIDE_PER_SESSION_MIB: u64 = 2;

const SESSION_COUNTS: [u64; 3] = [1, 5, 10];
const SAMPLES: usize = 5;
const SAMPLE_GAP: Duration = Duration::from_millis(500);
const DEADLINE: Duration = Duration::from_secs(90);

struct Runtime(Child);

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(name: &str) -> Runtime {
    let state = std::env::temp_dir().join(format!("brachio-state-{name}"));
    Runtime(
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
    let deadline = Instant::now() + DEADLINE;
    while client.call(Action::Hello).await.is_err() {
        assert!(Instant::now() < deadline, "runtime did not become ready");
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

/// Every process at or below `root`.
fn tree(system: &System, root: Pid) -> HashSet<Pid> {
    let mut pids = HashSet::from([root]);
    loop {
        let before = pids.len();
        for (pid, process) in system.processes() {
            if process
                .parent()
                .is_some_and(|parent| pids.contains(&parent))
            {
                pids.insert(*pid);
            }
        }
        if pids.len() == before {
            return pids;
        }
    }
}

/// Highest working set seen over a few samples, as bytes.
fn peak_working_sets(runtime: Pid) -> (u64, u64) {
    let mut system = System::new();
    let own = Pid::from_u32(std::process::id());
    let (mut runtime_peak, mut app_peak) = (0, 0);
    for _ in 0..SAMPLES {
        system.refresh_processes(ProcessesToUpdate::All, true);
        let runtime_total: u64 = tree(&system, runtime)
            .iter()
            .filter_map(|pid| system.process(*pid))
            .map(sysinfo::Process::memory)
            .sum();
        let app = system.process(own).map_or(0, sysinfo::Process::memory);
        runtime_peak = runtime_peak.max(runtime_total);
        app_peak = app_peak.max(app);
        std::thread::sleep(SAMPLE_GAP);
    }
    (runtime_peak, app_peak)
}

/// Reads a session's whole retained output, page by page, and drops it.
async fn read_all(client: &Client, key: &str) {
    let mut after = 0;
    let deadline = Instant::now() + DEADLINE;
    loop {
        let reply = client
            .call(Action::Read {
                key: key.to_owned(),
                after,
            })
            .await
            .expect("read output");
        let ResponseResult::Success {
            output: Some(page), ..
        } = reply.result
        else {
            panic!("unexpected reply to read");
        };
        if page.next >= page.end {
            return;
        }
        assert!(Instant::now() < deadline, "output did not finish");
        after = page.next;
    }
}

async fn run(count: u64) -> (u64, u64) {
    let name = format!("bound-{}", &Uuid::new_v4().simple().to_string()[..12]);
    let host = start(&name);
    let client = Client::new(&name);
    ready(&client).await;
    let keys: Vec<String> = (0..count).map(|index| format!("bound:{index}")).collect();
    for key in &keys {
        client
            .call(Action::Ensure {
                key: key.clone(),
                cwd: std::env::temp_dir(),
            })
            .await
            .expect("start shell");
        client
            .call(Action::Write {
                key: key.clone(),
                text: "for /L %i in (1,1,2500) do @echo bound-line-%i-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r"
                    .to_owned(),
            })
            .await
            .expect("write workload");
    }
    // Let the workloads finish and the output settle before reading.
    tokio::time::sleep(Duration::from_secs(5)).await;
    for key in &keys {
        read_all(&client, key).await;
    }
    let sampled = tokio::task::spawn_blocking({
        let runtime = Pid::from_u32(host.0.id());
        move || peak_working_sets(runtime)
    })
    .await
    .expect("sample memory");
    for key in &keys {
        let _ = client.call(Action::Stop { key: key.clone() }).await;
    }
    sampled
}

#[tokio::test]
#[ignore = "opt-in memory check; see the module comment"]
async fn working_set_stays_under_the_documented_bound() {
    let mut failures = Vec::new();
    println!("sessions,runtime_mib,runtime_bound_mib,app_side_mib,app_side_bound_mib");
    for count in SESSION_COUNTS {
        let (runtime, app) = run(count).await;
        let runtime_bound = RUNTIME_BASE_MIB + RUNTIME_PER_SESSION_MIB * count;
        let app_bound = APP_SIDE_BASE_MIB + APP_SIDE_PER_SESSION_MIB * count;
        println!(
            "{count},{},{runtime_bound},{},{app_bound}",
            runtime / MIB,
            app / MIB
        );
        if runtime > runtime_bound * MIB {
            failures.push(format!(
                "runtime tree with {count} sessions used {} MiB, over {runtime_bound} MiB",
                runtime / MIB
            ));
        }
        if app > app_bound * MIB {
            failures.push(format!(
                "app side with {count} sessions used {} MiB, over {app_bound} MiB",
                app / MIB
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
