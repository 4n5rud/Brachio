//! Memory and CPU of the app's process tree and the terminal runtimes, for the status bar.

use std::collections::HashSet;
use std::sync::Mutex;

use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Default)]
pub struct ResourceMonitor(Mutex<System>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    memory_bytes: u64,
    /// Part of `memory_bytes` from runtimes that are not below the app.
    runtime_memory_bytes: u64,
    cpu_percent: f32,
}

type ProcessRow = (Pid, Option<Pid>, String);

fn is_runtime_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("brachio-runtime") || name.eq_ignore_ascii_case("brachio-runtime.exe")
}

/// Every root and every process below any of them.
fn tree_pids(processes: &[ProcessRow], roots: impl IntoIterator<Item = Pid>) -> HashSet<Pid> {
    let mut pids: HashSet<Pid> = roots.into_iter().collect();
    loop {
        let before = pids.len();
        for (pid, parent, _) in processes {
            if parent.is_some_and(|parent| pids.contains(&parent)) {
                pids.insert(*pid);
            }
        }
        if pids.len() == before {
            return pids;
        }
    }
}

/// The app's tree plus the tree of every terminal runtime, which outlives the app.
fn counted_pids(processes: &[ProcessRow], app: Pid) -> HashSet<Pid> {
    let runtimes = processes
        .iter()
        .filter(|(_, _, name)| is_runtime_name(name))
        .map(|(pid, _, _)| *pid);
    tree_pids(processes, runtimes.chain([app]))
}

/// CPU is measured since the previous call, so the first reading is 0.
#[tauri::command]
pub fn resource_usage(monitor: tauri::State<'_, ResourceMonitor>) -> Result<ResourceUsage, String> {
    let mut system = monitor
        .0
        .lock()
        .map_err(|_| "resource monitor is poisoned".to_string())?;
    system.refresh_processes(ProcessesToUpdate::All, true);
    let processes: Vec<ProcessRow> = system
        .processes()
        .iter()
        .map(|(pid, process)| {
            (
                *pid,
                process.parent(),
                process.name().to_string_lossy().into_owned(),
            )
        })
        .collect();
    let app = Pid::from_u32(std::process::id());
    let pids = counted_pids(&processes, app);
    let app_tree = tree_pids(&processes, [app]);
    let (mut memory_bytes, mut runtime_memory_bytes, mut cpu_total) = (0u64, 0u64, 0f32);
    for pid in &pids {
        let Some(process) = system.process(*pid) else {
            continue;
        };
        memory_bytes += process.memory();
        cpu_total += process.cpu_usage();
        if !app_tree.contains(pid) {
            runtime_memory_bytes += process.memory();
        }
    }
    let cores = std::thread::available_parallelism().map_or(1, |count| count.get()) as f32;
    Ok(ResourceUsage {
        memory_bytes,
        runtime_memory_bytes,
        cpu_percent: (cpu_total / cores).clamp(0.0, 100.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, parent: Option<u32>, name: &str) -> ProcessRow {
        (
            Pid::from_u32(pid),
            parent.map(Pid::from_u32),
            name.to_string(),
        )
    }

    fn counted(processes: &[ProcessRow]) -> Vec<u32> {
        let mut pids: Vec<u32> = counted_pids(processes, Pid::from_u32(1))
            .iter()
            .map(|pid| pid.as_u32())
            .collect();
        pids.sort_unstable();
        pids
    }

    #[test]
    fn counts_app_children_and_detached_runtime_trees_only() {
        let processes = [
            row(1, Some(100), "brachio"),
            row(2, Some(1), "webview"),
            row(10, Some(200), "brachio-runtime.exe"),
            row(11, Some(10), "claude"),
            row(12, Some(11), "node"),
            row(20, Some(300), "chrome"),
            row(21, Some(20), "chrome-helper"),
            row(30, Some(300), "my-brachio-runtime-tool"),
            row(31, Some(30), "child"),
        ];
        assert_eq!(counted(&processes), vec![1, 2, 10, 11, 12]);
    }

    #[test]
    fn runtime_name_matches_case_insensitively_and_exactly() {
        let processes = [row(10, None, "BRACHIO-RUNTIME"), row(11, Some(10), "x")];
        assert_eq!(counted(&processes), vec![1, 10, 11]);
        assert!(!is_runtime_name("brachio-runtime-old"));
    }

    #[test]
    fn runtime_below_app_is_counted_once() {
        let processes = [
            row(1, None, "brachio"),
            row(10, Some(1), "brachio-runtime"),
            row(11, Some(10), "claude"),
        ];
        let pids = counted_pids(&processes, Pid::from_u32(1));
        assert_eq!(pids.len(), 3);
    }
}
