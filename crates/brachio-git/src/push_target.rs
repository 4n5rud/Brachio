//! Which remote a task branch is pushed to.

use crate::git::{Git, GitError};

/// The remote to push `branch` to: its own push remote, the repository's push default, the
/// remote it already tracks, the only remote there is, or `origin`.
///
/// # Errors
/// Fails when git cannot be run, or when there are several remotes and none of those rules
/// picks one.
pub fn push_remote(git: &Git, branch: &str) -> Result<String, GitError> {
    let configured = [
        format!("branch.{branch}.pushRemote"),
        "remote.pushDefault".to_owned(),
        format!("branch.{branch}.remote"),
    ]
    .into_iter()
    .find_map(|key| {
        git.run(&["config", "--get", &key])
            .ok()
            .filter(|value| !value.is_empty())
    });
    if let Some(remote) = configured {
        return Ok(remote);
    }
    let remotes = git.run(&["remote"])?;
    let names: Vec<&str> = remotes
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    match names.as_slice() {
        [only] => Ok((*only).to_owned()),
        names if names.contains(&"origin") => Ok("origin".to_owned()),
        [] => Err(GitError::Failed {
            args: "remote".to_owned(),
            status: 1,
            stderr: "This repository has no remote to push to.".to_owned(),
        }),
        _ => Err(GitError::Failed {
            args: "remote".to_owned(),
            status: 1,
            stderr: format!(
                "Several remotes ({}) and none is set for this branch. Run `git config branch.{branch}.pushRemote <name>`.",
                names.join(", ")
            ),
        }),
    }
}
