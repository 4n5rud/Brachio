//! Facts about a repository that Brachio needs when it is added.

use std::fs;
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
/// Fails with [`GitError::NotARepository`] when `path` is not inside a git repository. The error
/// lists repositories directly inside `path`, because a common mistake is choosing the folder
/// above the repository.
pub fn inspect_repo(path: &Path) -> Result<RepoInfo, GitError> {
    let root = match Git::new(path).run(&["rev-parse", "--show-toplevel"]) {
        Ok(root) => PathBuf::from(root),
        // The check is made on the folders, not on git's message, which can be translated.
        Err(GitError::Failed { .. }) if !inside_repository(path) => {
            return Err(GitError::NotARepository {
                path: path.to_path_buf(),
                suggestions: repositories_inside(path),
            });
        }
        Err(error) => return Err(error),
    };
    let git = Git::new(&root);
    let default_branch = remote_default_branch(&git)
        .or_else(|| current_branch(&git))
        .unwrap_or_else(|| "main".to_owned());
    Ok(RepoInfo {
        root,
        default_branch,
    })
}

const MAX_SUGGESTIONS: usize = 5;

fn inside_repository(path: &Path) -> bool {
    path.ancestors().any(|folder| folder.join(".git").exists())
}

fn repositories_inside(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|folder| folder.is_dir() && folder.join(".git").exists())
        .collect();
    found.sort();
    found.truncate(MAX_SUGGESTIONS);
    found
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
