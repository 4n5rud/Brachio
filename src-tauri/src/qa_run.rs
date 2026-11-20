//! Runs a task's project QA command after the user approves it in the approval bar.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use brachio_qa::{GatedOutcome, QaReport};
use brachio_runtime::approval::Decision;
use brachio_store::{RepoRecord, TaskRecord};
use tauri::State;

use crate::hook_socket::Approvals;
use crate::workspace::Database;

/// The last result of each task, kept until the app closes, and the tasks running now.
#[derive(Default)]
pub struct QaResults {
    last: Mutex<HashMap<String, QaReport>>,
    running: Mutex<HashSet<String>>,
}

struct Running<'a>(&'a QaResults, String);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        if let Ok(mut running) = self.0.running.lock() {
            running.remove(&self.1);
        }
    }
}

/// Asks the user to approve the project's QA command, runs it once if they do, and returns the
/// result. `None` when the user declined. Nothing is retried or merged.
#[tauri::command]
pub async fn qa_run(
    database: State<'_, Database>,
    approvals: State<'_, Arc<Approvals>>,
    results: State<'_, Arc<QaResults>>,
    task_id: String,
) -> Result<Option<QaReport>, String> {
    let (task, repo) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &task_id).map_err(|error| error.to_string())?;
        let repo =
            RepoRecord::get(&connection, &task.repo_id).map_err(|error| error.to_string())?;
        (task, repo)
    };
    if !repo.is_git {
        return Err("QA needs a git project.".to_owned());
    }
    let command = brachio_qa::read_command(&repo.path, &task.worktree_path)?
        .ok_or_else(|| "This project has no [qa] command in brachio.toml.".to_owned())?;
    let results = Arc::clone(&results);
    if !results
        .running
        .lock()
        .map_err(|_| "QA state lock poisoned".to_owned())?
        .insert(task_id.clone())
    {
        return Err("QA is already running for this task.".to_owned());
    }
    let approvals = Arc::clone(&approvals);
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let _running = Running(&results, task_id.clone());
        let key = format!("{task_id}:agent");
        let outcome = brachio_qa::run_after_approval(&command, &[], |review| {
            approvals.ask_blocking(key, review.to_owned()) == Some(Decision::Allow)
        })?;
        if let GatedOutcome::Ran(report) = &outcome {
            if let Ok(mut last) = results.last.lock() {
                last.insert(task_id, (**report).clone());
            }
        }
        Ok::<_, String>(outcome)
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(match outcome {
        GatedOutcome::Ran(report) => Some(*report),
        GatedOutcome::Declined => None,
    })
}

/// The last QA result of the task in this session.
#[tauri::command]
pub fn qa_last_result(results: State<'_, Arc<QaResults>>, task_id: String) -> Option<QaReport> {
    results.last.lock().ok()?.get(&task_id).cloned()
}

#[cfg(test)]
mod tests {
    use crate::hook_socket::Notice;

    use super::*;

    #[test]
    fn an_app_question_waits_for_the_users_answer_and_closes_afterwards() {
        let notices = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&notices);
        let approvals = Approvals::new(
            |_| true,
            move |notice| seen.lock().expect("lock").push(notice),
        );
        let asking = Arc::clone(&approvals);
        let thread =
            std::thread::spawn(move || asking.ask_blocking("t:agent".into(), "Run QA".into()));
        let id = loop {
            let first = notices.lock().expect("lock").first().cloned();
            if let Some(Notice::Asked(question)) = first {
                assert_eq!(question.review, "Run QA");
                break question.id;
            }
            std::thread::yield_now();
        };
        let denied = Decision::Deny {
            message: "no".into(),
        };
        assert!(approvals.answer(id, denied.clone()));
        assert_eq!(thread.join().expect("join"), Some(denied));
        assert_eq!(
            notices.lock().expect("lock").last(),
            Some(&Notice::Closed(id))
        );
    }
}
