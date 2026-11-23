//! A read-only view of a worktree's commit history: the list for a branch graph, and one
//! commit's patch.

use crate::git::{Git, GitError};

/// Most commits returned; older history is not listed.
pub const MAX_COMMITS: usize = 200;
const MAX_LISTING_BYTES: usize = 512 * 1024;
const MAX_PATCH_BYTES: usize = 256 * 1024;
const FIELD: char = '\x1f';
const RECORD: char = '\x1e';

/// One commit with the links the graph needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitEntry {
    /// Full commit hash.
    pub hash: String,
    /// Parent hashes, first parent first.
    pub parents: Vec<String>,
    /// Author name.
    pub author: String,
    /// Commit time, seconds since the epoch.
    pub time_secs: i64,
    /// Branch and tag names pointing at the commit.
    pub refs: Vec<String>,
    /// First line of the message.
    pub subject: String,
}

/// A commit's patch against its first parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPatch {
    /// Unified diff text.
    pub patch: String,
    /// The diff was cut at the size limit.
    pub truncated: bool,
}

/// Lists the commits reachable from `HEAD`, newest first, ordered so a parent always follows its
/// children.
///
/// # Errors
/// Fails when git cannot read the history. A repository without commits yields an empty list.
pub fn list_commits(git: &Git, limit: usize) -> Result<Vec<CommitEntry>, GitError> {
    if git
        .output(&["rev-parse", "--verify", "-q", "HEAD"])?
        .status
        .code()
        != Some(0)
    {
        return Ok(Vec::new());
    }
    let count = format!("-n{}", limit.min(MAX_COMMITS));
    let (listing, _) = git.run_text_capped(
        &[
            "log",
            "--topo-order",
            "--no-color",
            &count,
            "--format=%H%x1f%P%x1f%an%x1f%ct%x1f%D%x1f%s%x1e",
            "HEAD",
            "--",
        ],
        MAX_LISTING_BYTES,
    )?;
    Ok(parse_commits(&listing))
}

fn parse_commits(listing: &str) -> Vec<CommitEntry> {
    listing
        .split(RECORD)
        .filter_map(|record| {
            let mut fields = record.trim_start_matches('\n').split(FIELD);
            let hash = fields.next().filter(|hash| !hash.is_empty())?;
            let parents = fields.next()?;
            let author = fields.next()?;
            let time = fields.next()?;
            let refs = fields.next()?;
            let subject = fields.next()?;
            Some(CommitEntry {
                hash: hash.to_owned(),
                parents: parents.split_whitespace().map(str::to_owned).collect(),
                author: author.to_owned(),
                time_secs: time.parse().unwrap_or(0),
                refs: refs
                    .split(", ")
                    .filter_map(|name| name.strip_prefix("HEAD -> ").or(Some(name)))
                    .filter(|name| !name.is_empty() && *name != "HEAD")
                    .map(str::to_owned)
                    .collect(),
                subject: subject.to_owned(),
            })
        })
        .collect()
}

/// The patch a commit made over its first parent; a root commit is shown against nothing.
///
/// # Errors
/// Fails when `hash` is not a commit hash or git cannot read the commit.
pub fn commit_patch(git: &Git, hash: &str) -> Result<CommitPatch, GitError> {
    let valid = (7..=64).contains(&hash.len()) && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err(GitError::UnexpectedOutput(
            "That is not a commit hash.".to_owned(),
        ));
    }
    let (patch, truncated) = git.run_text_capped(
        &[
            "-c",
            "core.quotePath=false",
            "show",
            "--format=",
            "-m",
            "--first-parent",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--submodule=short",
            hash,
            "--",
        ],
        MAX_PATCH_BYTES,
    )?;
    Ok(CommitPatch { patch, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(git: &Git, args: &[&str]) {
        if let Err(error) = git.run(args) {
            panic!("git {args:?} failed: {error}");
        }
    }

    fn commit(git: &Git, file: &str, message: &str) {
        std::fs::write(git.repo().join(file), message).expect("write");
        run(git, &["add", "-A"]);
        run(
            git,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-q",
                "-m",
                message,
            ],
        );
    }

    #[test]
    fn parses_parents_and_refs() {
        let listing = "aaa\x1fbbb ccc\x1fAnn\x1f10\x1fHEAD -> main, tag: v1\x1fMerge\x1e\nbbb\x1f\x1fBob\x1f5\x1f\x1fRoot\x1e";
        let commits = parse_commits(listing);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents, ["bbb", "ccc"]);
        assert_eq!(commits[0].refs, ["main", "tag: v1"]);
        assert_eq!(commits[1].parents, Vec::<String>::new());
        assert_eq!(commits[1].subject, "Root");
    }

    #[test]
    fn lists_a_merge_and_shows_its_patch_against_the_first_parent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let git = Git::new(dir.path());
        run(&git, &["init", "-q", "-b", "main"]);
        assert!(list_commits(&git, 10).expect("empty").is_empty());
        commit(&git, "a.txt", "one");
        run(&git, &["checkout", "-q", "-b", "side"]);
        commit(&git, "b.txt", "two");
        run(&git, &["checkout", "-q", "main"]);
        commit(&git, "c.txt", "three");
        run(
            &git,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "merge",
                "-q",
                "--no-ff",
                "-m",
                "merge side",
                "side",
            ],
        );
        let commits = list_commits(&git, 10).expect("list");
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].subject, "merge side");
        assert_eq!(commits[0].parents.len(), 2);
        let patch = commit_patch(&git, &commits[0].hash).expect("patch");
        assert!(patch.patch.contains("b.txt"));
        assert!(!patch.patch.contains("c.txt"));
    }

    #[test]
    fn refuses_a_name_that_is_not_a_hash() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(commit_patch(&Git::new(dir.path()), "--output=x").is_err());
    }
}
