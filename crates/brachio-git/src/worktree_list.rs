//! The worktrees git knows for a repository, and which of them Brachio could take over as tasks.

use std::path::{Path, PathBuf};

use crate::git::{Git, GitError};

/// One entry of `git worktree list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    /// Folder of the worktree.
    pub path: PathBuf,
    /// Branch checked out, without `refs/heads/`; `None` for a detached HEAD.
    pub branch: Option<String>,
    /// A bare repository has no files to work in.
    pub bare: bool,
    /// Git is missing the folder, so the entry would be removed by `git worktree prune`.
    pub prunable: bool,
}

/// Why an import request was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImportError {
    /// The path is not a worktree of this repository that can become a task.
    #[error("{} is not a worktree of this repository that Brachio can import.", .0.display())]
    NotImportable(PathBuf),
}

/// Lists the repository's worktrees; the first entry is the main one.
///
/// # Errors
/// Fails when git cannot list the worktrees.
pub fn list_worktrees(git: &Git) -> Result<Vec<WorktreeEntry>, GitError> {
    let output = git.run_text(&["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_list(&output))
}

/// Parses `git worktree list --porcelain`: blocks of `key value` lines separated by blank lines.
#[must_use]
pub fn parse_worktree_list(output: &str) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    let mut current: Option<WorktreeEntry> = None;
    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            entries.extend(current.take());
            current = Some(WorktreeEntry {
                path: PathBuf::from(path),
                branch: None,
                bare: false,
                prunable: false,
            });
        } else if let Some(entry) = current.as_mut() {
            if let Some(branch) = line.strip_prefix("branch ") {
                entry.branch = Some(
                    branch
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch)
                        .to_owned(),
                );
            } else if line == "bare" {
                entry.bare = true;
            } else if line.starts_with("prunable") {
                entry.prunable = true;
            }
        }
    }
    entries.extend(current);
    entries
}

/// A key that is equal for two spellings of one folder: resolved when the folder exists, and
/// otherwise compared as written. Windows paths ignore case and separator style.
#[must_use]
pub fn path_key(path: &Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = resolved.to_string_lossy().replace('\\', "/");
    let text = text
        .strip_prefix("//?/")
        .unwrap_or(&text)
        .trim_end_matches('/')
        .to_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// Worktrees that could become tasks: not the main one, not bare or missing, on a branch, and not
/// in `known`.
#[must_use]
pub fn importable(entries: &[WorktreeEntry], known: &[PathBuf]) -> Vec<WorktreeEntry> {
    let known: Vec<String> = known.iter().map(|path| path_key(path)).collect();
    entries
        .iter()
        .skip(1)
        .filter(|entry| {
            !entry.bare
                && !entry.prunable
                && entry.branch.is_some()
                && entry.path.is_dir()
                && !known.contains(&path_key(&entry.path))
        })
        .cloned()
        .collect()
}

/// The entries the user asked to import. Each requested path must be one of the repository's own
/// importable worktrees, so a path from outside the repository is refused whole.
///
/// # Errors
/// Fails with the first requested path that is not importable; nothing is selected then.
pub fn select_importable(
    entries: &[WorktreeEntry],
    known: &[PathBuf],
    requested: &[PathBuf],
) -> Result<Vec<WorktreeEntry>, ImportError> {
    let candidates = importable(entries, known);
    let mut selected: Vec<WorktreeEntry> = Vec::new();
    for path in requested {
        let key = path_key(path);
        let entry = candidates
            .iter()
            .find(|entry| path_key(&entry.path) == key)
            .ok_or_else(|| ImportError::NotImportable(path.clone()))?;
        if !selected.contains(entry) {
            selected.push(entry.clone());
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &Path, branch: Option<&str>) -> WorktreeEntry {
        WorktreeEntry {
            path: path.to_path_buf(),
            branch: branch.map(str::to_owned),
            bare: false,
            prunable: false,
        }
    }

    #[test]
    fn the_porcelain_listing_is_parsed_into_entries() {
        let output = "worktree /src/app\nHEAD aaa\nbranch refs/heads/main\n\n\
                      worktree /src/app-hotfix\nHEAD bbb\nbranch refs/heads/fix/crash\n\n\
                      worktree /src/app-detached\nHEAD ccc\ndetached\n\n\
                      worktree /src/gone\nHEAD ddd\nbranch refs/heads/old\nprunable gitdir file points to non-existent location\n";
        let entries = parse_worktree_list(output);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[1].branch.as_deref(), Some("fix/crash"));
        assert_eq!(entries[1].path, PathBuf::from("/src/app-hotfix"));
        assert_eq!(entries[2].branch, None);
        assert!(entries[3].prunable);
    }

    #[test]
    fn only_untracked_branch_worktrees_that_exist_are_importable() {
        let folder = tempfile::tempdir().expect("temp dir");
        let make = |name: &str| {
            let path = folder.path().join(name);
            std::fs::create_dir(&path).expect("dir");
            path
        };
        let main = make("main");
        let hotfix = make("hotfix");
        let known = make("known");
        let detached = make("detached");
        let entries = vec![
            entry(&main, Some("main")),
            entry(&hotfix, Some("fix/crash")),
            entry(&known, Some("known")),
            entry(&detached, None),
            entry(&folder.path().join("missing"), Some("missing")),
        ];
        let found = importable(&entries, &[known]);
        assert_eq!(found, vec![entry(&hotfix, Some("fix/crash"))]);
    }

    #[test]
    fn a_path_outside_the_repository_worktrees_is_refused() {
        let folder = tempfile::tempdir().expect("temp dir");
        let main = folder.path().join("main");
        let hotfix = folder.path().join("hotfix");
        let outside = folder.path().join("elsewhere");
        for path in [&main, &hotfix, &outside] {
            std::fs::create_dir(path).expect("dir");
        }
        let entries = vec![entry(&main, Some("main")), entry(&hotfix, Some("fix"))];
        let chosen = select_importable(&entries, &[], &[hotfix.clone(), hotfix.clone()])
            .expect("importable");
        assert_eq!(chosen, vec![entry(&hotfix, Some("fix"))]);
        assert_eq!(
            select_importable(&entries, &[], &[hotfix, outside.clone()]),
            Err(ImportError::NotImportable(outside))
        );
        assert_eq!(
            select_importable(&entries, &[], std::slice::from_ref(&main)),
            Err(ImportError::NotImportable(main))
        );
    }
}
