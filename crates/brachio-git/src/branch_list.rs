//! Local and remote branches a new task can start from.

use crate::git::{Git, GitError};

/// Most branches returned; a repository with more is cut, newest first.
pub const MAX_BRANCHES: usize = 200;
const MAX_LISTING_BYTES: usize = 256 * 1024;

/// A branch to base a task on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchEntry {
    /// Name without the remote, as shown.
    pub name: String,
    /// What to hand to git as the base: the name for a local branch, `remote/name` otherwise.
    pub value: String,
    /// A local branch of this name exists.
    pub local: bool,
    /// Remote that has a branch of this name, if any.
    pub remote: Option<String>,
}

/// Lists local and remote-tracking branches, most recently committed first. A branch that exists
/// both locally and on a remote is one entry. Remote `HEAD` pointers are left out.
///
/// # Errors
/// Fails when git cannot read the repository's references.
pub fn list_branches(git: &Git) -> Result<Vec<BranchEntry>, GitError> {
    let (listing, _) = git.run_text_capped(
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
        MAX_LISTING_BYTES,
    )?;
    Ok(merge_refs(listing.lines()))
}

fn merge_refs<'a>(refs: impl Iterator<Item = &'a str>) -> Vec<BranchEntry> {
    let mut entries: Vec<BranchEntry> = Vec::new();
    for full in refs {
        let (name, remote) = if let Some(name) = full.strip_prefix("refs/heads/") {
            (name, None)
        } else if let Some(rest) = full.strip_prefix("refs/remotes/") {
            match rest.split_once('/') {
                Some((_, "HEAD")) | None => continue,
                Some((remote, name)) => (name, Some(remote)),
            }
        } else {
            continue;
        };
        if let Some(entry) = entries.iter_mut().find(|entry| entry.name == name) {
            match remote {
                None => {
                    entry.local = true;
                    name.clone_into(&mut entry.value);
                }
                Some(remote) => {
                    entry.remote.get_or_insert_with(|| remote.to_owned());
                }
            }
            continue;
        }
        entries.push(BranchEntry {
            name: name.to_owned(),
            value: remote.map_or_else(|| name.to_owned(), |remote| format!("{remote}/{name}")),
            local: remote.is_none(),
            remote: remote.map(str::to_owned),
        });
    }
    entries.truncate(MAX_BRANCHES);
    entries
}

#[cfg(test)]
mod tests {
    use super::merge_refs;

    #[test]
    fn local_and_remote_copies_are_one_entry_and_remote_head_is_skipped() {
        let refs = [
            "refs/heads/release/0.4",
            "refs/remotes/origin/release/0.4",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/release/0.3",
            "refs/heads/feat/relay-inbox",
            "refs/tags/v1",
        ];
        let entries = merge_refs(refs.into_iter());
        let summary: Vec<_> = entries
            .iter()
            .map(|entry| {
                (
                    entry.name.as_str(),
                    entry.value.as_str(),
                    entry.local,
                    entry.remote.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("release/0.4", "release/0.4", true, Some("origin")),
                ("release/0.3", "origin/release/0.3", false, Some("origin")),
                ("feat/relay-inbox", "feat/relay-inbox", true, None),
            ]
        );
    }

    #[test]
    fn a_remote_listed_before_its_local_copy_still_uses_the_local_name() {
        let entries = merge_refs(["refs/remotes/origin/a", "refs/heads/a"].into_iter());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value, "a");
        assert!(entries[0].local);
    }
}
