//! Memory and CPU of the app's process tree and this app's terminal runtimes, for the status bar.
//!
//! Memory is the sum of each counted process's resident memory (RSS on Unix, working set on
//! Windows) over the app tree and the trees of the runtimes this installation owns. Pages shared
//! between processes can be counted more than once, so the sum can exceed the unique memory in use.
//! The runtimes are the processes that answer `Hello` on this installation's own endpoints (current
//! and older generations); a runtime that does not answer or reports no PID is not counted, and
//! no process is picked by its name.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use brachio_runtime::client::Client;
use brachio_runtime::wire::{Action, ResponseResult};
use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::runtime_client;

const RUNTIME_ANSWER_TIMEOUT: Duration = Duration::from_millis(500);
const RUNTIME_PIDS_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct ResourceMonitor {
    system: Mutex<System>,
    runtime_pids: Mutex<Option<(Instant, Vec<Pid>)>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    memory_bytes: u64,
    /// Part of `memory_bytes` from runtimes that are not below the app.
    runtime_memory_bytes: u64,
    cpu_percent: f32,
}

type ProcessRow = (Pid, Option<Pid>);

/// Every root and every process below any of them.
fn tree_pids(processes: &[ProcessRow], roots: impl IntoIterator<Item = Pid>) -> HashSet<Pid> {
    let mut pids: HashSet<Pid> = roots.into_iter().collect();
    loop {
        let before = pids.len();
        for (pid, parent) in processes {
            if parent.is_some_and(|parent| pids.contains(&parent)) {
                pids.insert(*pid);
            }
        }
        if pids.len() == before {
            return pids;
        }
    }
}

/// The app's tree plus the tree of every known runtime, which outlives the app.
fn counted_pids(processes: &[ProcessRow], app: Pid, runtimes: &[Pid]) -> HashSet<Pid> {
    tree_pids(processes, runtimes.iter().copied().chain([app]))
}

/// PIDs of the runtimes that answer on this installation's endpoints; silent ones add nothing.
async fn answering_runtime_pids() -> Vec<Pid> {
    let mut pids = Vec::new();
    for endpoint in runtime_client::own_endpoints() {
        let client = Client::new(&endpoint);
        let call = client.call(Action::Hello);
        let Ok(Ok(reply)) = tokio::time::timeout(RUNTIME_ANSWER_TIMEOUT, call).await else {
            continue;
        };
        if let ResponseResult::Hello { runtime } = reply.result {
            let pid = Pid::from_u32(runtime.pid);
            if runtime.pid != 0 && !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }
    pids
}

impl ResourceMonitor {
    async fn runtime_pids(&self) -> Vec<Pid> {
        let cached = self.runtime_pids.lock().ok().and_then(|cache| {
            cache
                .as_ref()
                .filter(|(at, _)| at.elapsed() < RUNTIME_PIDS_TTL)
                .map(|(_, pids)| pids.clone())
        });
        if let Some(pids) = cached {
            return pids;
        }
        let pids = answering_runtime_pids().await;
        if let Ok(mut cache) = self.runtime_pids.lock() {
            *cache = Some((Instant::now(), pids.clone()));
        }
        pids
    }
}

/// CPU is measured since the previous call, so the first reading is 0.
#[tauri::command]
pub async fn resource_usage(
    monitor: tauri::State<'_, ResourceMonitor>,
) -> Result<ResourceUsage, String> {
    let runtimes = monitor.runtime_pids().await;
    let mut system = monitor
        .system
        .lock()
        .map_err(|_| "resource monitor is poisoned".to_string())?;
    system.refresh_processes(ProcessesToUpdate::All, true);
    let processes: Vec<ProcessRow> = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()))
        .collect();
    let app = Pid::from_u32(std::process::id());
    let pids = counted_pids(&processes, app, &runtimes);
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

    fn row(pid: u32, parent: Option<u32>) -> ProcessRow {
        (Pid::from_u32(pid), parent.map(Pid::from_u32))
    }

    fn counted(processes: &[ProcessRow], runtimes: &[u32]) -> Vec<u32> {
        let runtimes: Vec<Pid> = runtimes.iter().copied().map(Pid::from_u32).collect();
        let mut pids: Vec<u32> = counted_pids(processes, Pid::from_u32(1), &runtimes)
            .iter()
            .map(|pid| pid.as_u32())
            .collect();
        pids.sort_unstable();
        pids
    }

    #[test]
    fn counts_app_children_and_given_runtime_trees_only() {
        let processes = [
            row(1, Some(100)),
            row(2, Some(1)),
            row(10, Some(200)),
            row(11, Some(10)),
            row(12, Some(11)),
            row(30, Some(300)),
            row(31, Some(30)),
        ];
        assert_eq!(counted(&processes, &[10]), vec![1, 2, 10, 11, 12]);
    }

    #[test]
    fn current_and_old_runtimes_are_counted_once_each() {
        let processes = [
            row(1, None),
            row(10, Some(200)),
            row(11, Some(10)),
            row(20, Some(201)),
            row(21, Some(20)),
        ];
        assert_eq!(counted(&processes, &[10, 20]), vec![1, 10, 11, 20, 21]);
    }

    #[test]
    fn runtime_below_app_is_counted_once() {
        let processes = [row(1, None), row(10, Some(1)), row(11, Some(10))];
        assert_eq!(counted(&processes, &[10]), vec![1, 10, 11]);
    }

    #[test]
    fn no_known_runtime_counts_only_the_app() {
        let processes = [row(1, None), row(10, Some(200)), row(11, Some(10))];
        assert_eq!(counted(&processes, &[]), vec![1]);
    }
}
