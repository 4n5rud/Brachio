//! Creating and removing task worktrees.

use std::path::{Path, PathBuf};

use crate::git::{Git, GitError};

/// Inputs for `add_worktree`.
#[derive(Debug, Clone)]
pub struct AddWorktree {
    /// New branch to create.
    pub branch: String,
    /// Folder for the worktree. Must not exist yet.
    pub path: PathBuf,
    /// Commit-ish to branch from, for example `origin/main`.
    pub base: String,
}

/// `git worktree add -b <branch> <path> <base>`.
///
/// # Errors
/// Fails when the branch already exists, the path is taken, or the base is
/// unknown.
pub fn add_worktree(git: &Git, request: &AddWorktree) -> Result<(), GitError> {
    let path = request.path.to_string_lossy();
    git.run(&[
        "worktree",
        "add",
        "-b",
        &request.branch,
        &path,
        &request.base,
    ])
    .map(|_| ())
}

/// Removes a worktree and then its branch. git's own checks protect unsaved
/// work. Forcing is a separate, user-confirmed path.
///
/// # Errors
/// - The worktree has uncommitted changes: nothing is removed.
/// - The branch is unmerged: the worktree is removed, the branch stays.
pub fn remove_worktree(git: &Git, path: &Path, branch: &str) -> Result<(), GitError> {
    git.run(&["worktree", "remove", &path.to_string_lossy()])?;
    git.run(&["branch", "-d", branch]).map(|_| ())
}

/// Where the worktree for `branch` lives: a sibling folder of the repo named
/// `<repo>.worktrees`, so tools that scan the repo never see it.
#[must_use]
pub fn worktree_path(repo: &Path, branch: &str) -> PathBuf {
    let name = repo.file_name().map_or_else(
        || "repo".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let leaf = branch.rsplit('/').next().unwrap_or(branch);
    let parent = repo.parent().unwrap_or(repo);
    parent.join(format!("{name}.worktrees")).join(leaf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_sits_beside_the_repo() {
        let path = worktree_path(Path::new("/src/brachio"), "brachio/fix-login-a1b2c3");
        assert_eq!(
            path,
            PathBuf::from("/src/brachio.worktrees/fix-login-a1b2c3")
        );
    }
}
