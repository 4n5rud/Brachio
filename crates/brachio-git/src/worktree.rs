//! Creating and removing task worktrees.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

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
    // Use stable Git diagnostics when identifying the invalid-reference failure below.
    git.clone().with_env("LC_ALL", "C").run(&[
        "worktree",
        "add",
        "-b",
        &request.branch,
        &path,
        &request.base,
    ])
    .map(|_| ())
    .map_err(|error| {
        if matches!(&error, GitError::Failed { status: 128, stderr, .. } if stderr.contains("invalid reference:"))
            && base_is_unborn(git, &request.base)
        {
            GitError::InitialCommitRequired {
                path: git.repo().to_owned(),
                branch: request.base.clone(),
            }
        } else {
            error
        }
    })
}

fn base_is_unborn(git: &Git, base: &str) -> bool {
    let Ok(head) = git.run(&["symbolic-ref", "--quiet", "HEAD"]) else {
        return false;
    };
    let head = head.trim();
    if base != "HEAD" && base != head && Some(base) != head.strip_prefix("refs/heads/") {
        return false;
    }
    let reference = format!("{base}^{{commit}}");
    matches!(
        git.run(&["rev-parse", "--verify", "--quiet", &reference]),
        Err(GitError::Failed { status: 1, .. })
    )
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
    /// names and contents of untracked and ignored entries. A later difference means the user saw
    /// stale facts.
    pub fingerprint: String,
}

/// How many ignored names are returned as examples.
const IGNORED_EXAMPLES: usize = 5;
const MAX_UNTRACKED_LIST_BYTES: usize = 1024 * 1024;
const MAX_UNTRACKED_ENTRIES: usize = 20_000;
const MAX_UNTRACKED_CONTENT_BYTES: u64 = 256 * 1024 * 1024;

/// Counts what deleting the task would lose, without changing anything. A worktree folder that is
/// already gone has no uncommitted files.
///
/// # Errors
/// Fails when git cannot read the repository or the untracked contents cannot be fingerprinted
/// within the scan limits.
pub fn unsaved_work(git: &Git, path: &Path, branch: &str) -> Result<UnsavedWork, GitError> {
    let mut hash = Fnv1a::new();
    let mut status = StatusEntries::default();
    let mut ignored = IgnoredEntries::default();
    let mut tracked_changes = Fnv1a::new();
    if path.is_dir() {
        let worktree = Git::new(path);
        fingerprint_untracked(&worktree, &mut hash)?;
        stream_records(
            &worktree,
            &["status", "--porcelain", "--untracked-files=normal", "-z"],
            |record| {
                status.add(record);
            },
        )?;
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
            |chunk| tracked_changes.update(chunk),
        )?;
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

fn fingerprint_untracked(git: &Git, hash: &mut Fnv1a) -> Result<(), GitError> {
    // Without exclude rules Git lists both untracked and ignored entries. Walk collapsed
    // directories below to include every file inside them, including empty directories.
    let (output, truncated) = git.run_bytes_capped(
        &["ls-files", "--others", "--directory", "-z"],
        MAX_UNTRACKED_LIST_BYTES,
    )?;
    if truncated {
        return Err(scan_limit());
    }
    let mut pending: Vec<_> = output
        .split(|&byte| byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| resolve_entry(git.repo(), entry))
        .collect::<Result<_, _>>()?;
    if pending.len() > MAX_UNTRACKED_ENTRIES {
        return Err(scan_limit());
    }
    pending.sort_unstable();
    pending.reverse();
    let mut remaining_entries = MAX_UNTRACKED_ENTRIES;
    let mut remaining_bytes = MAX_UNTRACKED_CONTENT_BYTES;
    let mut buffer = [0; 8192];
    while let Some(path) = pending.pop() {
        remaining_entries = remaining_entries.checked_sub(1).ok_or_else(scan_limit)?;
        let relative = path
            .strip_prefix(git.repo())
            .map_err(|_| unreadable_entry(&path))?;
        hash.bytes(relative.as_os_str().as_encoded_bytes());
        let metadata = fs::symlink_metadata(&path)?;
        let kind = metadata.file_type();
        if kind.is_symlink() {
            hash.field("symlink");
            hash.bytes(fs::read_link(&path)?.as_os_str().as_encoded_bytes());
        } else if kind.is_dir() {
            hash.field("directory");
            let mut children = Vec::new();
            for child in fs::read_dir(&path)? {
                if pending.len() + children.len() >= remaining_entries {
                    return Err(scan_limit());
                }
                children.push(child?.path());
            }
            children.sort_unstable();
            pending.extend(children.into_iter().rev());
        } else if kind.is_file() {
            hash.field("file");
            let len = metadata.len();
            remaining_bytes = remaining_bytes.checked_sub(len).ok_or_else(scan_limit)?;
            hash.update(&len.to_le_bytes());
            let mut file = File::open(&path)?.take(len + 1);
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hash.update(&buffer[..read]);
            }
            if file.limit() != 1 {
                return Err(unreadable_entry(&path));
            }
        } else {
            return Err(unreadable_entry(&path));
        }
    }
    Ok(())
}

fn resolve_entry(root: &Path, entry: &[u8]) -> Result<PathBuf, GitError> {
    let entry = entry.strip_suffix(b"/").unwrap_or(entry);
    #[cfg(unix)]
    let name = <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(entry);
    #[cfg(not(unix))]
    let name = std::str::from_utf8(entry)
        .map_err(|_| GitError::UnexpectedOutput("could not decode an untracked git path".into()))?;
    let mut path = root.to_owned();
    let mut components = Path::new(name).components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(GitError::UnexpectedOutput(
                "unsafe untracked path from git".into(),
            ));
        };
        path.push(name);
        if components.peek().is_some() && !fs::symlink_metadata(&path)?.is_dir() {
            return Err(unreadable_entry(&path));
        }
    }
    Ok(path)
}

fn scan_limit() -> GitError {
    GitError::UnexpectedOutput(
        "Cannot safely confirm deletion: untracked and ignored files exceed the scan limits. Move or remove them before trying again.".into(),
    )
}

fn unreadable_entry(path: &Path) -> GitError {
    GitError::UnexpectedOutput(format!(
        "Cannot safely confirm deletion: could not fingerprint {}.",
        path.display()
    ))
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

    /// Mixes a length-prefixed field so adjacent fields cannot run together.
    fn field(&mut self, text: &str) {
        self.bytes(text.as_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.update(&(bytes.len() as u64).to_le_bytes());
        self.update(bytes);
    }

    fn update(&mut self, bytes: &[u8]) {
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
