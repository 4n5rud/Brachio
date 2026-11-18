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

/// Work that deleting a task would destroy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsavedWork {
    /// Files with uncommitted changes, including untracked ones.
    pub uncommitted_files: u32,
    /// Commits only the task branch holds: no remote branch and no other local branch has them.
    pub unpushed_commits: u32,
}

/// Counts what deleting the task would lose, without changing anything. A worktree folder that is
/// already gone has no uncommitted files.
///
/// # Errors
/// Fails when git cannot read the repository.
pub fn unsaved_work(git: &Git, path: &Path, branch: &str) -> Result<UnsavedWork, GitError> {
    let uncommitted_files = if path.is_dir() {
        let status = Git::new(path).run_text(&["status", "--porcelain", "-z"])?;
        count(status.split_terminator('\0').count())
    } else {
        0
    };
    let reference = format!("refs/heads/{branch}");
    // Commits another branch or a remote also holds survive the deletion, so they are not counted.
    let exclude = format!("--exclude={branch}");
    let unpushed = git.run(&[
        "rev-list",
        "--count",
        &reference,
        "--not",
        "--remotes",
        &exclude,
        "--branches",
    ])?;
    Ok(UnsavedWork {
        uncommitted_files,
        unpushed_commits: unpushed.parse().unwrap_or(0),
    })
}

/// Removes the worktree and its branch whatever they hold. The caller must have shown
/// `unsaved_work` to the user first. A worktree folder that is already gone is pruned instead.
///
/// # Errors
/// Fails when git refuses, for example when the branch is checked out somewhere else.
pub fn discard_worktree(git: &Git, path: &Path, branch: &str) -> Result<(), GitError> {
    if path.is_dir() {
        git.run(&["worktree", "remove", "--force", &path.to_string_lossy()])?;
    } else {
        git.run(&["worktree", "prune"])?;
    }
    git.run(&["branch", "-D", branch]).map(|_| ())
}

fn count(entries: usize) -> u32 {
    u32::try_from(entries).unwrap_or(u32::MAX)
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
