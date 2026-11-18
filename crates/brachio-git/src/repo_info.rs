//! Facts about a repository that Brachio needs when it is added.

use std::path::{Path, PathBuf};

use crate::git::{Git, GitError};

/// The root and default branch of a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    /// Top-level folder of the working tree.
    pub root: PathBuf,
    /// Branch new tasks start from.
    pub default_branch: String,
}

/// Reads the root and default branch of the repository that contains `path`.
///
/// The default branch is the one `origin/HEAD` points at. Without a remote,
/// it is the branch currently checked out.
///
/// # Errors
/// Fails when `path` is not inside a git repository.
pub fn inspect_repo(path: &Path) -> Result<RepoInfo, GitError> {
    let root = PathBuf::from(Git::new(path).run(&["rev-parse", "--show-toplevel"])?);
    let git = Git::new(&root);
    let default_branch = remote_default_branch(&git)
        .or_else(|| current_branch(&git))
        .unwrap_or_else(|| "main".to_owned());
    Ok(RepoInfo {
        root,
        default_branch,
    })
}

fn remote_default_branch(git: &Git) -> Option<String> {
    let reference = git
        .run(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .ok()?;
    let branch = reference.strip_prefix("origin/").unwrap_or(&reference);
    (!branch.is_empty()).then(|| branch.to_owned())
}

fn current_branch(git: &Git) -> Option<String> {
    let branch = git.run(&["branch", "--show-current"]).ok()?;
    (!branch.is_empty()).then_some(branch)
}
