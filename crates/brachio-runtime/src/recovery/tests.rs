use std::path::PathBuf;
use std::time::{Duration, Instant};

use brachio_runtime::protocol;
use brachio_runtime::wire::{Action, Request};

use super::*;

fn launch_exiting(runtime: &Runtime, key: &str) {
    let (program, args) = if cfg!(windows) {
        ("cmd.exe", vec!["/d", "/c", "echo done"])
    } else {
        ("sh", vec!["-c", "echo done"])
    };
    runtime
        .execute(Request {
            version: protocol::CURRENT,
            action: Action::Launch {
                key: key.to_owned(),
                cwd: std::env::temp_dir(),
                program: program.to_owned(),
                args: args.into_iter().map(str::to_owned).collect(),
                env: Vec::new(),
                rows: 24,
                cols: 80,
            },
        })
        .expect("launch");
}

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("brachio-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

#[test]
fn the_idle_tick_reaps_programs_that_exited_and_keeps_their_last_screen() {
    let directory = scratch("reap");
    let runtime = Runtime::new(Some(Journal::open(&directory).expect("open journal")));
    launch_exiting(&runtime, "task/exits");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !lock(&runtime.sessions).expect("sessions").is_empty() {
        runtime.maintain();
        assert!(Instant::now() < deadline, "ended program was never reaped");
        std::thread::sleep(Duration::from_millis(25));
    }
    let lost = lock(&runtime.lost).expect("lost");
    assert!(lost.get("task/exits").is_some_and(|record| record.exited));
    drop(lost);
    let on_disk = runtime
        .journal
        .as_ref()
        .expect("journal")
        .load()
        .expect("load");
    assert!(on_disk
        .records
        .iter()
        .any(|record| record.key == "task/exits" && record.exited));
    let _ = std::fs::remove_dir_all(directory);
}
