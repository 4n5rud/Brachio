//! The files of a worktree, for jumping to one by name.

use crate::git::{Git, GitError};

/// Files longer lists are cut at; a repository this large would make the list slow to search.
pub const MAX_FILES: usize = 20_000;

/// Lists tracked files and untracked files that git does not ignore, as paths relative to the
/// worktree with forward slashes, sorted.
///
/// # Errors
/// Fails when git cannot read the worktree.
pub fn list_files(git: &Git) -> Result<Vec<String>, GitError> {
    let listing = git.run_text(&[
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ])?;
    let mut files: Vec<String> = listing
        .split_terminator('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();
    // git lists tracked and untracked files one after the other, and a path can appear in both.
    files.sort();
    files.dedup();
    files.truncate(MAX_FILES);
    Ok(files)
}
