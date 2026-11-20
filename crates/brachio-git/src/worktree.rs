//! Creating and removing task worktrees.

use std::path::{Path, PathBuf};

use crate::git::{Git, GitError};
use crate::nul_records::NulRecords;

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
    let mut status = StatusEntries::default();
    let mut ignored = IgnoredEntries::default();
    let mut tracked_changes = Fnv1a::new();
    if path.is_dir() {
        let worktree = Git::new(path);
        stream_records(&worktree, &["status", "--porcelain", "-z"], |record| {
            status.add(record);
        })?;
        stream_records(
            &worktree,
            &[
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
                "-z",
            ],
            |record| ignored.add(record),
        )?;
        worktree.stream_stdout(
            &["diff", "HEAD", "--no-ext-diff", "--no-textconv"],
            |chunk| tracked_changes.bytes(chunk),
        )?;
        tracked_changes.bytes(&[SEPARATOR]);
    }
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
    let uncommitted_files = count(status.entries);
    let ignored_entries = count(ignored.entries);
    // Entries are summed rather than sorted because they are streamed, so their order is free.
    let mut hash = Fnv1a::new();
    hash.field(&uncommitted_files.to_string());
    hash.field(&unpushed_commits.to_string());
    hash.field(&ignored_entries.to_string());
    for example in &ignored.examples {
        hash.field(example);
    }
    hash.field(&head);
    hash.bytes(&tracked_changes.0.to_le_bytes());
    hash.bytes(&status.fingerprint.to_le_bytes());
    hash.bytes(&ignored.fingerprint.to_le_bytes());
    Ok(UnsavedWork {
        uncommitted_files,
        unpushed_commits,
        ignored_entries,
        ignored_examples: ignored.examples,
        fingerprint: hash.finish(),
    })
}

/// Feeds each NUL-terminated record of a git command to `on_record` without collecting them.
fn stream_records(
    git: &Git,
    args: &[&str],
    mut on_record: impl FnMut(&str),
) -> Result<(), GitError> {
    let mut records = NulRecords::default();
    git.stream_stdout(args, |chunk| records.push(chunk, &mut on_record))?;
    records
        .finish(on_record)
        .map_err(GitError::UnexpectedOutput)
}

/// Counts `git status --porcelain -z` entries. A rename or copy is one entry that spans two
/// records: the new path, then the original path.
#[derive(Default)]
struct StatusEntries {
    entries: usize,
    fingerprint: u64,
    open: Option<Fnv1a>,
}

impl StatusEntries {
    fn add(&mut self, record: &str) {
        if let Some(mut entry) = self.open.take() {
            entry.field(record);
            self.close(&entry);
            return;
        }
        let mut entry = Fnv1a::new();
        entry.field(record);
        let moved = record
            .bytes()
            .take(2)
            .any(|code| matches!(code, b'R' | b'C'));
        if moved {
            self.open = Some(entry);
        } else {
            self.close(&entry);
        }
    }

    fn close(&mut self, entry: &Fnv1a) {
        self.entries += 1;
        self.fingerprint = self.fingerprint.wrapping_add(entry.0);
    }
}

/// Counts ignored entries and keeps the first few names.
#[derive(Default)]
struct IgnoredEntries {
    entries: usize,
    fingerprint: u64,
    examples: Vec<String>,
}

impl IgnoredEntries {
    fn add(&mut self, record: &str) {
        self.entries += 1;
        let mut entry = Fnv1a::new();
        entry.field(record);
        self.fingerprint = self.fingerprint.wrapping_add(entry.0);
        if self.examples.len() < IGNORED_EXAMPLES {
            self.examples.push(record.to_owned());
        }
    }
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

const SEPARATOR: u8 = 0xff;

impl Fnv1a {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    /// Mixes a field and a separator so adjacent fields cannot run together.
    fn field(&mut self, text: &str) {
        self.bytes(text.as_bytes());
        self.bytes(&[SEPARATOR]);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
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
