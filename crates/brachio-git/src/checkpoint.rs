//! Restore points: file-only snapshots of a task's worktree, kept as hidden commits under
//! `refs/brachio/checkpoints/<task>/<n>`. A snapshot is built in a throwaway index, so the
//! user's branch, index and HEAD are never touched.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::git::{Git, GitError};

/// Most restore points kept per task; the oldest are dropped first.
pub const MAX_CHECKPOINTS: usize = 50;
/// Largest patch returned for one restore point.
const MAX_DIFF_BYTES: usize = 1024 * 1024;
const REF_ROOT: &str = "refs/brachio/checkpoints";
const ZERO_OID: &str = "";

/// Why a snapshot was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    /// The agent is about to start a turn, so this keeps the user's own edits apart from the agent's.
    TurnStart,
    /// The agent finished a turn.
    TurnEnd,
    /// The state just before a restore overwrote the files.
    BeforeRestore,
}

impl CaptureKind {
    fn code(self) -> &'static str {
        match self {
            Self::TurnStart => "turn_start",
            Self::TurnEnd => "turn_end",
            Self::BeforeRestore => "before_restore",
        }
    }
}

/// One stored restore point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// Position in the task's history; grows by one for each new snapshot.
    pub number: u32,
    /// The hidden commit holding the snapshot.
    pub commit: String,
    /// When it was taken, in Unix seconds.
    pub created_at_secs: i64,
    /// A `CaptureKind` code, or whatever an older version wrote.
    pub kind: String,
    /// Files changed since the previous restore point.
    pub files: u32,
    /// Lines added since the previous restore point.
    pub additions: u32,
    /// Lines removed since the previous restore point.
    pub deletions: u32,
}

/// What one restore point changed compared with the one before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointDiff {
    /// The unified diff, possibly cut at a size limit.
    pub patch: String,
    /// Whether the patch was cut.
    pub truncated: bool,
    /// The restore point compared against; none for the oldest.
    pub base_number: Option<u32>,
}

/// The outcome of a restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The restore point holding the files as they were just before the restore; restoring it undoes this one.
    pub backup_number: u32,
}

fn rejected(message: impl Into<String>) -> GitError {
    GitError::UnexpectedOutput(message.into())
}

fn ref_prefix(task_id: &str) -> Result<String, GitError> {
    let valid = !task_id.is_empty()
        && task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(rejected("The task id cannot name restore points."));
    }
    Ok(format!("{REF_ROOT}/{task_id}"))
}

/// A throwaway index file that is removed when dropped.
struct TempIndex(PathBuf);

impl TempIndex {
    fn create(git: &Git, seed_from_real_index: bool) -> Result<Self, GitError> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let git_dir = PathBuf::from(git.run(&["rev-parse", "--absolute-git-dir"])?);
        let name = format!(
            "brachio-checkpoint-{}-{}.index",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = git_dir.join(name);
        // The copy keeps git's cached file stats, so unchanged files are not hashed again.
        let real = git_dir.join("index");
        if seed_from_real_index && real.is_file() {
            fs::copy(&real, &path)?;
        }
        Ok(Self(path))
    }

    fn git(&self, git: &Git) -> Git {
        git.clone()
            .with_env("GIT_INDEX_FILE", &self.0.to_string_lossy())
    }
}

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let mut lock = self.0.clone().into_os_string();
        lock.push(".lock");
        let _ = fs::remove_file(lock);
    }
}

fn identity(git: &Git) -> Git {
    git.clone()
        .with_env("GIT_AUTHOR_NAME", "Brachio")
        .with_env("GIT_AUTHOR_EMAIL", "brachio@localhost")
        .with_env("GIT_COMMITTER_NAME", "Brachio")
        .with_env("GIT_COMMITTER_EMAIL", "brachio@localhost")
}

/// Lists a task's restore points, oldest first.
///
/// # Errors
/// Fails when the task id is invalid or git fails.
pub fn list_checkpoints(git: &Git, task_id: &str) -> Result<Vec<Checkpoint>, GitError> {
    let prefix = ref_prefix(task_id)?;
    let format =
        "--format=%(refname)%00%(objectname)%00%(committerdate:unix)%00%(contents:subject)";
    let pattern = format!("{prefix}/");
    let text = git.run_text(&["for-each-ref", format, &pattern])?;
    let mut found: Vec<Checkpoint> = text
        .lines()
        .filter_map(|line| parse_row(line, &pattern))
        .collect();
    found.sort_by_key(|checkpoint| checkpoint.number);
    Ok(found)
}

fn parse_row(line: &str, pattern: &str) -> Option<Checkpoint> {
    let mut parts = line.split('\0');
    let number = parts.next()?.strip_prefix(pattern)?.parse().ok()?;
    let commit = parts.next()?.to_owned();
    let created_at_secs = parts.next()?.parse().ok()?;
    let mut words = parts.next()?.split_whitespace();
    let (Some("checkpoint"), Some(kind)) = (words.next(), words.next()) else {
        return None;
    };
    let mut stat = |sign: &str| -> u32 {
        words
            .next()
            .and_then(|word| word.strip_prefix(sign))
            .and_then(|count| count.parse().ok())
            .unwrap_or(0)
    };
    let files = stat("files=");
    let additions = stat("+");
    let deletions = stat("-");
    Some(Checkpoint {
        number,
        commit,
        created_at_secs,
        kind: kind.to_owned(),
        files,
        additions,
        deletions,
    })
}

struct Taken {
    number: u32,
    tree: String,
    created: Option<Checkpoint>,
}

/// Snapshots the worktree, including untracked files and skipping ignored ones. Returns the new
/// restore point, or none when nothing changed since the latest one.
///
/// # Errors
/// Fails when the task id is invalid or git fails.
pub fn capture_checkpoint(
    git: &Git,
    task_id: &str,
    kind: CaptureKind,
) -> Result<Option<Checkpoint>, GitError> {
    let taken = take(git, task_id, kind)?;
    prune_to(git, task_id, MAX_CHECKPOINTS)?;
    Ok(taken.created)
}

fn take(git: &Git, task_id: &str, kind: CaptureKind) -> Result<Taken, GitError> {
    let prefix = ref_prefix(task_id)?;
    let existing = list_checkpoints(git, task_id)?;
    let index = TempIndex::create(git, true)?;
    let indexed = index.git(git);
    indexed.run(&["add", "-A", "--", "."])?;
    let tree = indexed.run(&["write-tree"])?;

    let latest = existing.last();
    let latest_tree = latest
        .map(|checkpoint| git.run(&["rev-parse", &format!("{}^{{tree}}", checkpoint.commit)]))
        .transpose()?;
    if let (Some(latest), Some(latest_tree)) = (latest, &latest_tree) {
        if *latest_tree == tree {
            return Ok(Taken {
                number: latest.number,
                tree,
                created: None,
            });
        }
    }

    let (files, additions, deletions) = match &latest_tree {
        Some(base) => change_counts(git, base, &tree)?,
        None => (0, 0, 0),
    };
    let subject = format!(
        "checkpoint {} files={files} +{additions} -{deletions}",
        kind.code()
    );
    let commit = identity(git).run(&[
        "-c",
        "commit.gpgsign=false",
        "commit-tree",
        &tree,
        "-m",
        &subject,
    ])?;
    let number = latest.map_or(1, |checkpoint| checkpoint.number + 1);
    let reference = format!("{prefix}/{number}");
    git.run(&["update-ref", &reference, &commit, ZERO_OID])?;
    let created_at_secs = git
        .run(&["show", "-s", "--format=%ct", &commit])?
        .parse()
        .unwrap_or(0);
    Ok(Taken {
        number,
        tree,
        created: Some(Checkpoint {
            number,
            commit,
            created_at_secs,
            kind: kind.code().to_owned(),
            files,
            additions,
            deletions,
        }),
    })
}

fn change_counts(git: &Git, from: &str, to: &str) -> Result<(u32, u32, u32), GitError> {
    let text = git.run_text(&["diff-tree", "-r", "--numstat", from, to])?;
    let (mut files, mut additions, mut deletions) = (0, 0, 0);
    for line in text.lines() {
        let mut parts = line.split('\t');
        let (Some(added), Some(removed)) = (parts.next(), parts.next()) else {
            continue;
        };
        files += 1;
        // Binary files report "-", which counts as a changed file with no lines.
        additions += added.parse::<u32>().unwrap_or(0);
        deletions += removed.parse::<u32>().unwrap_or(0);
    }
    Ok((files, additions, deletions))
}

fn prune_to(git: &Git, task_id: &str, limit: usize) -> Result<(), GitError> {
    let prefix = ref_prefix(task_id)?;
    let all = list_checkpoints(git, task_id)?;
    let excess = all.len().saturating_sub(limit);
    delete_refs(git, &prefix, all.iter().take(excess).map(|c| c.number))
}

fn delete_refs(
    git: &Git,
    prefix: &str,
    numbers: impl Iterator<Item = u32>,
) -> Result<(), GitError> {
    let commands: String = numbers
        .map(|number| format!("delete {prefix}/{number}\n"))
        .collect();
    if commands.is_empty() {
        return Ok(());
    }
    git.run_with_input(&["update-ref", "--stdin"], commands.as_bytes())
        .map(drop)
}

/// Removes every restore point of a task, for when the task is archived or deleted.
///
/// # Errors
/// Fails when the task id is invalid or git fails.
pub fn delete_checkpoints(git: &Git, task_id: &str) -> Result<(), GitError> {
    let prefix = ref_prefix(task_id)?;
    let all = list_checkpoints(git, task_id)?;
    delete_refs(git, &prefix, all.iter().map(|c| c.number))
}

/// The change one restore point made over the one before it.
///
/// # Errors
/// Fails when the restore point does not exist or git fails.
pub fn checkpoint_diff(git: &Git, task_id: &str, number: u32) -> Result<CheckpointDiff, GitError> {
    let all = list_checkpoints(git, task_id)?;
    let position = all
        .iter()
        .position(|checkpoint| checkpoint.number == number)
        .ok_or_else(|| rejected("That restore point no longer exists."))?;
    let Some(base) = position.checked_sub(1).map(|before| &all[before]) else {
        return Ok(CheckpointDiff {
            patch: String::new(),
            truncated: false,
            base_number: None,
        });
    };
    let (patch, truncated) = git.run_text_capped(
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            &base.commit,
            &all[position].commit,
        ],
        MAX_DIFF_BYTES,
    )?;
    Ok(CheckpointDiff {
        patch,
        truncated,
        base_number: Some(base.number),
    })
}

/// Puts the worktree's files back as a restore point has them. A backup restore point is taken
/// first, so the restore can be undone. The branch, index and HEAD are not touched.
///
/// # Errors
/// Fails when the restore point does not exist or git or the file system fails.
pub fn restore_checkpoint(
    git: &Git,
    worktree: &Path,
    task_id: &str,
    number: u32,
) -> Result<Restored, GitError> {
    let target = list_checkpoints(git, task_id)?
        .into_iter()
        .find(|checkpoint| checkpoint.number == number)
        .ok_or_else(|| rejected("That restore point no longer exists."))?;
    let target_tree = git.run(&["rev-parse", &format!("{}^{{tree}}", target.commit)])?;
    let backup = take(git, task_id, CaptureKind::BeforeRestore)?;

    let stale = git.run_text(&[
        "diff-tree",
        "-r",
        "-z",
        "--name-only",
        "--diff-filter=A",
        &target_tree,
        &backup.tree,
    ])?;
    for name in stale.split('\0').filter(|name| !name.is_empty()) {
        remove_file_and_empty_parents(worktree, name)?;
    }

    let index = TempIndex::create(git, false)?;
    let indexed = index.git(git);
    indexed.run(&["read-tree", &target_tree])?;
    indexed.run(&["checkout-index", "-a", "-f"])?;

    prune_to(git, task_id, MAX_CHECKPOINTS)?;
    Ok(Restored {
        backup_number: backup.number,
    })
}

fn remove_file_and_empty_parents(worktree: &Path, relative: &str) -> Result<(), GitError> {
    let path = Path::new(relative);
    let safe = path
        .components()
        .all(|part| matches!(part, Component::Normal(name) if !name.eq_ignore_ascii_case(".git")));
    if !safe {
        return Err(rejected(
            "A restore point names a path that cannot be changed.",
        ));
    }
    let file = worktree.join(path);
    match fs::remove_file(&file) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut folder = file.parent();
    while let Some(dir) = folder.filter(|dir| *dir != worktree && dir.starts_with(worktree)) {
        // Only an empty folder can be removed, which is the stop signal.
        if fs::remove_dir(dir).is_err() {
            break;
        }
        folder = dir.parent();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(git: &Git, args: &[&str]) {
        if let Err(error) = git.run(args) {
            panic!("git {args:?} failed: {error}");
        }
    }

    #[test]
    fn oldest_restore_points_are_pruned_past_the_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let git = Git::new(dir.path());
        run(&git, &["init", "-q", "-b", "main"]);
        for round in 0..5 {
            fs::write(dir.path().join("a.txt"), format!("{round}\n")).expect("write");
            take(&git, "t1", CaptureKind::TurnEnd).expect("take");
            prune_to(&git, "t1", 3).expect("prune");
        }
        let numbers: Vec<u32> = list_checkpoints(&git, "t1")
            .expect("list")
            .iter()
            .map(|checkpoint| checkpoint.number)
            .collect();
        assert_eq!(numbers, [3, 4, 5]);
    }
}
