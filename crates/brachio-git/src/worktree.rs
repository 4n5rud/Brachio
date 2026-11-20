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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsavedWork {
    /// Files with uncommitted changes, including untracked ones.
    pub uncommitted_files: u32,
    /// Commits only the task branch holds: no remote branch and no other local branch has them.
    pub unpushed_commits: u32,
    /// Files and folders git ignores, such as `.env.local`, notes and build output. They were
    /// never committed, so nothing else holds them.
    pub ignored_entries: u32,
    /// Some of their names, top level only.
    pub ignored_examples: Vec<String>,
    /// Identifies exactly this state: the facts above, the branch head, tracked changes and the
    /// names of every changed or ignored entry. A later difference means the user saw stale facts.
    pub fingerprint: String,
}

/// How many ignored names are returned as examples.
const IGNORED_EXAMPLES: usize = 5;

/// Counts what deleting the task would lose, without changing anything. A worktree folder that is
/// already gone has no uncommitted files.
///
/// # Errors
/// Fails when git cannot read the repository.
pub fn unsaved_work(git: &Git, path: &Path, branch: &str) -> Result<UnsavedWork, GitError> {
    let (mut status_entries, mut ignored, tracked_changes) = if path.is_dir() {
        let worktree = Git::new(path);
        let status = worktree.run_text(&["status", "--porcelain", "-z"])?;
        let ignored = worktree.run_text(&[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ])?;
        let tracked = worktree.run_text(&["diff", "HEAD", "--no-ext-diff", "--no-textconv"])?;
        (names(&status), names(&ignored), tracked)
    } else {
        (Vec::new(), Vec::new(), String::new())
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
    let unpushed_commits = parse_count(&unpushed).map_err(GitError::UnexpectedOutput)?;
    let head = git.run(&["rev-parse", "--verify", &reference])?;
    let uncommitted_files = count(status_entries.len());
    let ignored_entries = count(ignored.len());
    let ignored_examples: Vec<String> = ignored.iter().take(IGNORED_EXAMPLES).cloned().collect();
    status_entries.sort();
    ignored.sort();
    let mut hash = Fnv1a::new();
    hash.field(&uncommitted_files.to_string());
    hash.field(&unpushed_commits.to_string());
    hash.field(&ignored_entries.to_string());
    for example in &ignored_examples {
        hash.field(example);
    }
    hash.field(&head);
    hash.field(&tracked_changes);
    for entry in status_entries.iter().chain(&ignored) {
        hash.field(entry);
    }
    Ok(UnsavedWork {
        uncommitted_files,
        unpushed_commits,
        ignored_entries,
        ignored_examples,
        fingerprint: hash.finish(),
    })
}

fn names(nul_separated: &str) -> Vec<String> {
    nul_separated
        .split_terminator('\0')
        .map(str::to_owned)
        .collect()
}

/// Parses a git count. Anything else is an error so an unknown count never reads as zero.
fn parse_count(text: &str) -> Result<u32, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("could not count unpushed commits: unexpected output {text:?}"))
}

/// FNV-1a 64-bit. `DefaultHasher` is not stable across runs and no hashing crate is a dependency
/// here; the fingerprint only needs to differ when the state differs, not to resist attackers.
struct Fnv1a(u64);

impl Fnv1a {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    /// Mixes a field and a separator so adjacent fields cannot run together.
    fn field(&mut self, text: &str) {
        for byte in text.bytes().chain([0xff]) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finish(&self) -> String {
        format!("{:016x}", self.0)
    }
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
    fn a_count_that_is_not_a_number_is_an_error_not_zero() {
        assert_eq!(parse_count("3\n"), Ok(3));
        assert!(parse_count("").is_err());
        assert!(parse_count("warning: 3").is_err());
        assert!(parse_count("-1").is_err());
    }

    #[test]
    fn worktree_sits_beside_the_repo() {
        let path = worktree_path(Path::new("/src/brachio"), "brachio/fix-login-a1b2c3");
        assert_eq!(
            path,
            PathBuf::from("/src/brachio.worktrees/fix-login-a1b2c3")
        );
    }
}
