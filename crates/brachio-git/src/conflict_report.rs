//! Dry-run merges that predict conflicts between task branches.

use crate::git::{Git, GitError};

/// Result of a dry-run merge between two branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictReport {
    /// Paths that would conflict, as git prints them.
    pub conflicted_files: Vec<String>,
}

impl ConflictReport {
    /// True when the merge would apply without conflicts.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.conflicted_files.is_empty()
    }
}

/// Predicts whether merging `ours` and `theirs` conflicts, without touching any
/// working tree. Needs git 2.38 or newer.
///
/// `git merge-tree --write-tree --name-only` exits 0 when clean and 1 when
/// conflicted; its stdout is the tree id, then one conflicted path per line,
/// then a blank line and informational messages.
///
/// # Errors
/// Fails when git cannot run or `merge-tree` reports something other than
/// clean or conflicted, for example an unknown branch.
pub fn predict_conflict(git: &Git, ours: &str, theirs: &str) -> Result<ConflictReport, GitError> {
    let output = git.output(&["merge-tree", "--write-tree", "--name-only", ours, theirs])?;
    match output.status.code() {
        Some(0) => Ok(ConflictReport {
            conflicted_files: vec![],
        }),
        Some(1) => Ok(parse_conflicts(&String::from_utf8_lossy(&output.stdout))),
        code => Err(GitError::Failed {
            args: format!("merge-tree --write-tree --name-only {ours} {theirs}"),
            status: code.unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }),
    }
}

fn parse_conflicts(stdout: &str) -> ConflictReport {
    let conflicted_files = stdout
        .lines()
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    ConflictReport { conflicted_files }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paths_between_tree_id_and_blank_line() {
        let stdout = "4b825dc6\nsrc/a.rs\nsrc/b.rs\n\nAuto-merging src/a.rs\n";
        let report = parse_conflicts(stdout);
        assert_eq!(report.conflicted_files, vec!["src/a.rs", "src/b.rs"]);
    }
}
