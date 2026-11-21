//! Whether git is in the middle of a rebase or merge in a worktree.

use std::path::PathBuf;

use crate::git::Git;

/// A git operation that stopped and waits for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoppedOperation {
    /// `rebase-merge` or `rebase-apply` exists in the git directory.
    Rebase,
    /// `MERGE_HEAD` exists in the git directory.
    Merge,
}

impl StoppedOperation {
    /// The name the interface switches on.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rebase => "rebase",
            Self::Merge => "merge",
        }
    }
}

/// The rebase or merge that is in progress in the worktree, if any. A worktree has its own git
/// directory, so another task's rebase is never reported. Failures read as "none".
#[must_use]
pub fn stopped_operation(git: &Git) -> Option<StoppedOperation> {
    let dir = git.run(&["rev-parse", "--absolute-git-dir"]).ok()?;
    operation_in(&PathBuf::from(dir))
}

fn operation_in(git_dir: &std::path::Path) -> Option<StoppedOperation> {
    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        Some(StoppedOperation::Rebase)
    } else if git_dir.join("MERGE_HEAD").exists() {
        Some(StoppedOperation::Merge)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{operation_in, StoppedOperation};

    #[test]
    fn rebase_folders_and_merge_head_are_told_apart() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(operation_in(dir.path()), None);
        std::fs::write(dir.path().join("MERGE_HEAD"), "x").expect("write");
        assert_eq!(operation_in(dir.path()), Some(StoppedOperation::Merge));
        std::fs::create_dir(dir.path().join("rebase-merge")).expect("mkdir");
        assert_eq!(operation_in(dir.path()), Some(StoppedOperation::Rebase));
    }
}
