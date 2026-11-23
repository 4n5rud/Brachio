//! Remembers a task worktree between creating it and saving the task, so a stop in between does
//! not leave a worktree that no task owns.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use brachio_git::Git;
use serde::{Deserialize, Serialize};

static FOLDER: OnceLock<PathBuf> = OnceLock::new();

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingWorktree {
    pub(crate) task_id: String,
    pub(crate) repo_path: PathBuf,
    pub(crate) worktree_path: PathBuf,
    pub(crate) branch: String,
}

/// Where the records live; set once the app data folder is known. Until then nothing is recorded.
pub(crate) fn init(folder: PathBuf) {
    let _ = FOLDER.set(folder);
}

fn record_path(folder: &Path, task_id: &str) -> PathBuf {
    folder.join(format!("{task_id}.json"))
}

pub(crate) fn record(pending: &PendingWorktree) {
    if let Some(folder) = FOLDER.get() {
        if let Err(error) = record_in(folder, pending) {
            tracing::warn!(%error, "could not record the worktree being created");
        }
    }
}

pub(crate) fn clear(task_id: &str) {
    if let Some(folder) = FOLDER.get() {
        if let Err(error) = clear_in(folder, task_id) {
            tracing::warn!(%error, "could not clear the record of a created worktree");
        }
    }
}

fn record_in(folder: &Path, pending: &PendingWorktree) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    brachio_fs::write_atomic(
        &record_path(folder, &pending.task_id),
        &serde_json::to_vec(pending).map_err(io::Error::other)?,
    )
}

fn clear_in(folder: &Path, task_id: &str) -> io::Result<()> {
    match fs::remove_file(record_path(folder, task_id)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Removes the worktree and branch of a task that was never saved. git refuses when the worktree
/// holds changes, so nothing a person wrote is lost; a folder that was never created counts as done.
fn roll_back(pending: &PendingWorktree) -> Result<(), String> {
    if !pending.worktree_path.exists() {
        return Ok(());
    }
    brachio_git::remove_worktree(
        &Git::new(&pending.repo_path),
        &pending.worktree_path,
        &pending.branch,
    )
    .map_err(|error| error.to_string())
}

/// Finishes what a stop left half done: a recorded worktree whose task was saved is kept, one whose
/// task was not is removed. A record is dropped after one try; a
/// worktree that could not be removed stays on disk and shows up in the import list.
pub(crate) fn recover(folder: &Path, saved: impl Fn(&PendingWorktree) -> bool) {
    recover_with(folder, saved, roll_back)
}

fn recover_with(
    folder: &Path,
    saved: impl Fn(&PendingWorktree) -> bool,
    roll_back: impl Fn(&PendingWorktree) -> Result<(), String>,
) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let Some(pending) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<PendingWorktree>(&bytes).ok())
        else {
            let _ = brachio_fs::quarantine_corrupt(&path);
            continue;
        };
        if !saved(&pending) {
            match roll_back(&pending) {
                Ok(()) => {}
                Err(error) => tracing::warn!(
                    %error,
                    path = %pending.worktree_path.display(),
                    "could not remove the worktree of a task that was never saved"
                ),
            }
        }
        let _ = fs::remove_file(&path);
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use brachio_git::{add_worktree, AddWorktree};

    use super::*;

    fn git(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(args)
            .current_dir(cwd)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?}");
    }

    /// A repository and a worktree created for a task whose row was never saved.
    fn stopped_after_the_worktree(folder: &Path, base: &Path) -> PendingWorktree {
        let repo = base.join("repo");
        fs::create_dir_all(&repo).expect("repo folder");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "start"]);
        let pending = PendingWorktree {
            task_id: "abc123".to_owned(),
            repo_path: repo.clone(),
            worktree_path: base.join("repo.worktrees").join("task-abc123"),
            branch: "brachio/task-abc123".to_owned(),
        };
        record_in(folder, &pending).expect("record first");
        add_worktree(
            &Git::new(&repo),
            &AddWorktree {
                branch: pending.branch.clone(),
                path: pending.worktree_path.clone(),
                base: "main".to_owned(),
            },
        )
        .expect("add worktree");
        pending
    }

    #[test]
    fn a_worktree_whose_task_was_never_saved_is_removed_on_the_next_start() {
        let base = tempfile::tempdir().expect("temp dir");
        let folder = base.path().join("pending");
        let pending = stopped_after_the_worktree(&folder, base.path());

        recover(&folder, |_| false);

        assert!(!pending.worktree_path.exists());
        assert!(fs::read_dir(&folder).expect("folder").next().is_none());
    }

    #[test]
    fn a_worktree_whose_task_was_saved_is_kept() {
        let base = tempfile::tempdir().expect("temp dir");
        let folder = base.path().join("pending");
        let pending = stopped_after_the_worktree(&folder, base.path());

        recover(&folder, |_| true);

        assert!(pending.worktree_path.exists());
        assert!(fs::read_dir(&folder).expect("folder").next().is_none());
    }

    #[test]
    fn a_worktree_with_changes_stays_and_the_record_is_dropped() {
        let base = tempfile::tempdir().expect("temp dir");
        let folder = base.path().join("pending");
        let pending = stopped_after_the_worktree(&folder, base.path());
        fs::write(pending.worktree_path.join("notes.txt"), "mine").expect("write");

        recover(&folder, |_| false);

        assert!(pending.worktree_path.join("notes.txt").exists());
        assert!(fs::read_dir(&folder).expect("folder").next().is_none());
    }

    #[test]
    fn a_record_for_a_worktree_never_created_is_just_dropped() {
        let base = tempfile::tempdir().expect("temp dir");
        let folder = base.path().join("pending");
        record_in(
            &folder,
            &PendingWorktree {
                task_id: "x".to_owned(),
                repo_path: base.path().join("none"),
                worktree_path: base.path().join("none.worktrees").join("x"),
                branch: "b".to_owned(),
            },
        )
        .expect("record");
        recover(&folder, |_| false);
        assert!(fs::read_dir(&folder).expect("folder").next().is_none());
    }
}
