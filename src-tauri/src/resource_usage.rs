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
use crate::runtime_terminals::RuntimeTerminals;
use crate::workspace::Database;

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

/// One process in a task's terminal trees.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProcess {
    pid: u32,
    parent_pid: Option<u32>,
    name: String,
    /// Terminal key of the tree the process belongs to.
    key: String,
    /// Percent of one core since the previous reading; the first reading is 0.
    cpu_percent: f32,
    memory_bytes: u64,
}

/// The task's live terminals and the pids of their shells.
async fn task_roots(
    database: &Database,
    terminals: &RuntimeTerminals,
    task_id: &str,
) -> Result<Vec<(String, u32)>, String> {
    let tabs: Vec<String> = {
        let connection = database.lock()?;
        brachio_store::task_tabs(&connection, task_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|tab| tab.tab_id)
            .collect()
    };
    terminals
        .live_sessions_for_task(task_id, &tabs)
        .await
        .map_err(|error| format!("{error:#}"))
}

/// CPU and memory of every process below the task's terminals, shells included.
#[tauri::command]
pub async fn task_processes(
    monitor: tauri::State<'_, ResourceMonitor>,
    database: tauri::State<'_, Database>,
    terminals: tauri::State<'_, RuntimeTerminals>,
    id: String,
) -> Result<Vec<TaskProcess>, String> {
    let roots = task_roots(&database, &terminals, &id).await?;
    let mut system = monitor
        .system
        .lock()
        .map_err(|_| "resource monitor is poisoned".to_string())?;
    system.refresh_processes(ProcessesToUpdate::All, true);
    let rows: Vec<ProcessRow> = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()))
        .collect();
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for (key, root) in roots {
        for pid in tree_pids(&rows, [Pid::from_u32(root)]) {
            let Some(process) = system.process(pid) else {
                continue;
            };
            if !seen.insert(pid) {
                continue;
            }
            found.push(TaskProcess {
                pid: pid.as_u32(),
                parent_pid: process.parent().map(Pid::as_u32),
                name: process.name().to_string_lossy().into_owned(),
                key: key.clone(),
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
            });
        }
    }
    found.sort_by_key(|process| process.pid);
    Ok(found)
}

/// Ends one process, only when it is in the task's terminal trees.
#[tauri::command]
pub async fn task_process_kill(
    monitor: tauri::State<'_, ResourceMonitor>,
    database: tauri::State<'_, Database>,
    terminals: tauri::State<'_, RuntimeTerminals>,
    id: String,
    pid: u32,
) -> Result<(), String> {
    let roots = task_roots(&database, &terminals, &id).await?;
    let mut system = monitor
        .system
        .lock()
        .map_err(|_| "resource monitor is poisoned".to_string())?;
    system.refresh_processes(ProcessesToUpdate::All, true);
    let rows: Vec<ProcessRow> = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()))
        .collect();
    let target = Pid::from_u32(pid);
    let inside = roots
        .iter()
        .any(|(_, root)| tree_pids(&rows, [Pid::from_u32(*root)]).contains(&target));
    if !inside {
        return Err("That process is not part of this task.".to_owned());
    }
    match system.process(target) {
        Some(process) if process.kill() => Ok(()),
        Some(_) => Err("The process could not be ended.".to_owned()),
        None => Ok(()),
    }
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
