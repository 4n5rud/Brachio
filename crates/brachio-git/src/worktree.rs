use std::path::PathBuf;

use crate::cli::{Git, GitError};

/// Inputs for `add_worktree`.
#[derive(Debug, Clone)]
pub struct AddWorktree {
    pub branch: String,
    pub path: PathBuf,
    /// Commit-ish to branch from, for example `origin/main`.
    pub base: String,
}

/// `git worktree add -b <branch> <path> <base>`.
pub fn add_worktree(git: &Git, req: &AddWorktree) -> Result<(), GitError> {
    let path = req.path.to_string_lossy();
    git.run(&["worktree", "add", "-b", &req.branch, &path, &req.base])
        .map(|_| ())
}

/// Removes a worktree and its branch. Refuses (git's own check) when the
/// worktree has uncommitted changes or the branch is unmerged; forcing is a
/// separate, user-confirmed path.
pub fn remove_worktree(git: &Git, path: &std::path::Path, branch: &str) -> Result<(), GitError> {
    git.run(&["worktree", "remove", &path.to_string_lossy()])?;
    git.run(&["branch", "-d", branch]).map(|_| ())
}
