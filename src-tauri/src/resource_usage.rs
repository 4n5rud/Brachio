//! Memory and CPU of the app's own process tree, for the status bar.

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
    cpu_percent: f32,
}

/// Pids of `root` and every process below it.
fn tree_pids(system: &System, root: Pid) -> HashSet<Pid> {
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

/// CPU is measured since the previous call, so the first reading is 0.
#[tauri::command]
pub fn resource_usage(monitor: tauri::State<'_, ResourceMonitor>) -> Result<ResourceUsage, String> {
    let mut system = monitor
        .0
        .lock()
        .map_err(|_| "resource monitor is poisoned".to_string())?;
    system.refresh_processes(ProcessesToUpdate::All, true);
    let pids = tree_pids(&system, Pid::from_u32(std::process::id()));
    let (memory_bytes, cpu_total) = pids
        .iter()
        .filter_map(|pid| system.process(*pid))
        .fold((0u64, 0f32), |(memory, cpu), process| {
            (memory + process.memory(), cpu + process.cpu_usage())
        });
    let cores = std::thread::available_parallelism().map_or(1, |count| count.get()) as f32;
    Ok(ResourceUsage {
        memory_bytes,
        cpu_percent: (cpu_total / cores).clamp(0.0, 100.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_contains_own_process_and_reports_memory() {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        let me = Pid::from_u32(std::process::id());
        assert!(tree_pids(&system, me).contains(&me));
        assert!(system
            .process(me)
            .is_some_and(|process| process.memory() > 0));
    }
}
