//! Archives a task whose pull request is merged, when nothing would be left behind. The user
//! turns this on in settings; the worktree and branch are never touched.

use brachio_git::Git;
use brachio_store::record_activity;
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::pr_status::PrStatuses;
use crate::workspace::{
    archive_unless_children_open, lock_open_git_task, now_ms, Database, TaskWorktreeLocks,
    OPEN_CHILDREN_PREFIX,
};

/// What happened to a task that looked ready to archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoArchive {
    /// The task is archived.
    Archived,
    /// Its pull request is not known to be merged.
    NotMerged,
    /// The worktree holds changes that are not committed.
    Uncommitted,
    /// It leads child tasks that are still open.
    HasOpenChildren,
}

/// Archives the task when its pull request `merged_pr` is merged and `porcelain`, the output of
/// `git status --porcelain`, is empty. Records why it was archived.
fn archive_merged(
    connection: &Connection,
    id: &str,
    merged_pr: Option<u64>,
    porcelain: &str,
    at_ms: i64,
) -> Result<AutoArchive, String> {
    let Some(number) = merged_pr else {
        return Ok(AutoArchive::NotMerged);
    };
    if !porcelain.trim().is_empty() {
        return Ok(AutoArchive::Uncommitted);
    }
    match archive_unless_children_open(connection, id, at_ms) {
        Ok(()) => {}
        Err(refusal) if refusal.starts_with(OPEN_CHILDREN_PREFIX) => {
            return Ok(AutoArchive::HasOpenChildren)
        }
        Err(error) => return Err(error),
    }
    let detail = format!("pull request #{number} merged");
    if let Err(error) = record_activity(connection, id, "auto_archived", Some(&detail), at_ms) {
        tracing::warn!(%error, "could not record the automatic archive");
    }
    tracing::info!(
        task = id,
        pull_request = number,
        "archived a task whose pull request is merged"
    );
    Ok(AutoArchive::Archived)
}

/// Archives the task if its pull request is merged and its worktree has no uncommitted changes.
/// The caller checks that no agent of the task is working.
#[tauri::command]
pub async fn task_archive_if_merged(
    database: State<'_, Database>,
    worktrees: State<'_, TaskWorktreeLocks>,
    statuses: State<'_, PrStatuses>,
    id: String,
) -> Result<AutoArchive, String> {
    let merged_pr = statuses.merged_number(&id)?;
    if merged_pr.is_none() {
        return Ok(AutoArchive::NotMerged);
    }
    // Held so a git action or a deletion cannot change the worktree between the check and the archive.
    let (task, _guard) = lock_open_git_task(&database, &worktrees, &id).await?;
    let path = task.worktree_path.clone();
    let porcelain = tauri::async_runtime::spawn_blocking(move || {
        Git::new(&path).run(&["status", "--porcelain", "--untracked-files=normal"])
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    let outcome = archive_merged(&*database.lock()?, &id, merged_pr, &porcelain, now_ms())?;
    if outcome == AutoArchive::Archived {
        crate::task_checkpoints::forget_for_task(&database, &id);
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use brachio_store::{open_in_memory, task_activity, NewRepo, RepoRecord};
    use std::path::Path;

    use super::{archive_merged, AutoArchive};

    fn database() -> rusqlite::Connection {
        let connection = open_in_memory().expect("open database");
        let repo = NewRepo {
            id: "r",
            name: "r",
            path: Path::new("/repo"),
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(&connection, &repo).expect("insert repo");
        connection
            .execute_batch(
                "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms, parent_task_id)
                 VALUES ('t', 'r', 'T', '', 'b', '/w', 'active', 0, NULL),
                        ('lead', 'r', 'L', '', 'l', '/l', 'active', 0, NULL),
                        ('kid', 'r', 'K', '', 'k', '/k', 'active', 0, 'lead');",
            )
            .expect("insert tasks");
        connection
    }

    fn archived(connection: &rusqlite::Connection, id: &str) -> bool {
        connection
            .query_row(
                "SELECT archived_at_ms IS NOT NULL FROM tasks WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .expect("read task")
    }

    #[test]
    fn only_a_merged_pull_request_with_a_clean_worktree_archives_and_the_reason_is_recorded() {
        let connection = database();
        assert_eq!(
            archive_merged(&connection, "t", None, "", 1).expect("check"),
            AutoArchive::NotMerged
        );
        assert_eq!(
            archive_merged(&connection, "t", Some(7), " M src/lib.rs\n", 2).expect("check"),
            AutoArchive::Uncommitted
        );
        assert_eq!(
            archive_merged(&connection, "t", Some(7), "?? notes.txt", 3).expect("check"),
            AutoArchive::Uncommitted
        );
        assert!(!archived(&connection, "t"));
        assert_eq!(
            archive_merged(&connection, "t", Some(7), "", 4).expect("check"),
            AutoArchive::Archived
        );
        assert!(archived(&connection, "t"));
        let events = task_activity(&connection, "t").expect("activity");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "auto_archived");
        assert_eq!(events[0].detail.as_deref(), Some("pull request #7 merged"));
    }

    #[test]
    fn a_lead_with_open_children_stays() {
        let connection = database();
        assert_eq!(
            archive_merged(&connection, "lead", Some(1), "", 1).expect("check"),
            AutoArchive::HasOpenChildren
        );
        assert!(!archived(&connection, "lead"));
    }
}
