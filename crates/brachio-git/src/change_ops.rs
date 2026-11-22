//! Throws away changes to one file or one hunk of the worktree, for the Changes panel.

use std::path::{Component, Path};

use crate::git::{Git, GitError};

/// Largest patch accepted for a hunk discard; a hunk is a few lines, never megabytes.
const MAX_HUNK_PATCH: usize = 256 * 1024;

fn rejected(message: &str) -> GitError {
    GitError::UnexpectedOutput(message.to_owned())
}

/// Whether the commit `HEAD` has this file, so a discard can restore it.
///
/// # Errors
/// Fails when git cannot be started.
pub fn in_head(git: &Git, path: &str) -> Result<bool, GitError> {
    let spec = format!("HEAD:{path}");
    Ok(git.output(&["cat-file", "-e", &spec])?.status.success())
}

/// Puts a file back as `HEAD` has it, in the index and the working tree.
///
/// # Errors
/// Fails when the file is not in `HEAD` or git fails.
pub fn discard_tracked(git: &Git, path: &str) -> Result<(), GitError> {
    if !in_head(git, path)? {
        return Err(rejected("The file is new, so there is nothing to restore."));
    }
    git.run(&[
        "--literal-pathspecs",
        "restore",
        "--source=HEAD",
        "--staged",
        "--worktree",
        "--",
        path,
    ])
    .map(drop)
}

/// The file a one-file modification patch is about, if the patch is safe to reverse-apply.
fn patched_path(patch: &str) -> Result<&str, GitError> {
    let mut headers = patch.lines().filter(|line| line.starts_with("diff --git "));
    let header = headers
        .next()
        .ok_or_else(|| rejected("The hunk has no file header."))?;
    if headers.next().is_some() {
        return Err(rejected("Discard one file's hunk at a time."));
    }
    let names = &header["diff --git ".len()..];
    let length = names
        .len()
        .checked_sub(5)
        .map(|rest| rest / 2)
        .ok_or_else(|| rejected("The hunk header is not understood."))?;
    let path = names
        .get(2..2 + length)
        .filter(|path| names == format!("a/{path} b/{path}"))
        .ok_or_else(|| rejected("This file name cannot be discarded by hunk."))?;
    let clean = !path.is_empty()
        && !path.contains(['"', '\\'])
        && Path::new(path).components().all(
            |part| matches!(part, Component::Normal(name) if !name.eq_ignore_ascii_case(".git")),
        );
    if !clean {
        return Err(rejected("The hunk is for a path that cannot be changed."));
    }
    let unsupported = patch.lines().any(|line| {
        [
            "new file mode",
            "deleted file mode",
            "rename ",
            "copy ",
            "Binary files",
            "GIT binary patch",
            "old mode",
            "new mode",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
    });
    if unsupported || !patch.lines().any(|line| line.starts_with("@@ ")) {
        return Err(rejected(
            "Only edits to existing text files can be discarded by hunk.",
        ));
    }
    Ok(path)
}

/// Reverses one hunk in the working tree. `patch` is the file header and that one hunk; nothing
/// changes unless the whole of it still applies.
///
/// # Errors
/// Fails when the patch is not a single-file text edit, no longer applies, or git fails.
pub fn discard_hunk(git: &Git, patch: &str) -> Result<(), GitError> {
    if patch.len() > MAX_HUNK_PATCH {
        return Err(rejected("The hunk is too large to discard."));
    }
    patched_path(patch)?;
    let body = if patch.ends_with('\n') {
        patch.to_owned()
    } else {
        format!("{patch}\n")
    };
    let flags = ["apply", "--reverse", "--recount", "--whitespace=nowarn"];
    let check = [flags.as_slice(), &["--check", "-"]].concat();
    git.run_with_input(&check, body.as_bytes())?;
    let apply = [flags.as_slice(), &["-"]].concat();
    git.run_with_input(&apply, body.as_bytes()).map(drop)
}

#[cfg(test)]
mod tests {
    use super::{discard_hunk, discard_tracked, in_head};
    use crate::git::Git;
    use std::fs;

    fn repo() -> (tempfile::TempDir, Git) {
        let dir = tempfile::tempdir().expect("temp dir");
        let git = Git::new(dir.path());
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "t"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.autocrlf", "false"],
        ] {
            git.run(&args).expect("git setup");
        }
        let lines: Vec<String> = (1..=30).map(|n| format!("line {n}")).collect();
        fs::write(dir.path().join("a.txt"), lines.join("\n") + "\n").expect("write");
        fs::write(dir.path().join("b.txt"), "b\n").expect("write");
        git.run(&["add", "."]).expect("add");
        git.run(&["commit", "-q", "-m", "init"]).expect("commit");
        (dir, git)
    }

    #[test]
    fn discards_a_tracked_file_from_the_index_and_tree() {
        let (dir, git) = repo();
        fs::write(dir.path().join("b.txt"), "changed\n").expect("edit");
        git.run(&["add", "b.txt"]).expect("stage");
        fs::write(dir.path().join("b.txt"), "changed again\n").expect("edit");
        assert!(in_head(&git, "b.txt").expect("head"));
        discard_tracked(&git, "b.txt").expect("discard");
        assert_eq!(
            fs::read_to_string(dir.path().join("b.txt")).expect("read"),
            "b\n"
        );
        assert_eq!(git.run(&["status", "--porcelain"]).expect("status"), "");
    }

    #[test]
    fn a_new_file_is_not_in_head() {
        let (dir, git) = repo();
        fs::write(dir.path().join("new.txt"), "n\n").expect("write");
        git.run(&["add", "new.txt"]).expect("stage");
        assert!(!in_head(&git, "new.txt").expect("head"));
        assert!(discard_tracked(&git, "new.txt").is_err());
        assert!(dir.path().join("new.txt").is_file());
    }

    #[test]
    fn discards_only_the_chosen_hunk() {
        let (dir, git) = repo();
        let mut text = fs::read_to_string(dir.path().join("a.txt")).expect("read");
        text = text
            .replace("line 2\n", "second\n")
            .replace("line 28\n", "twenty-eight\n");
        fs::write(dir.path().join("a.txt"), &text).expect("edit");
        let diff = git.run_text(&["diff", "-U1", "--", "a.txt"]).expect("diff");
        let at = diff.find("@@").expect("hunk");
        let second = diff[at + 2..]
            .find("@@ -")
            .map(|offset| at + 2 + offset)
            .expect("second hunk");
        let header = &diff[..at];
        let patch = format!("{header}{}", &diff[second..]);
        discard_hunk(&git, &patch).expect("discard");
        let after = fs::read_to_string(dir.path().join("a.txt")).expect("read");
        assert!(
            after.contains("second\n")
                && after.contains("line 28\n")
                && !after.contains("twenty-eight")
        );
    }

    #[test]
    fn refuses_patches_that_are_not_one_text_edit() {
        let (dir, git) = repo();
        fs::write(dir.path().join("a.txt"), "changed\n").expect("edit");
        let diff = git.run_text(&["diff", "--", "a.txt"]).expect("diff");
        assert!(discard_hunk(&git, "").is_err());
        assert!(discard_hunk(&git, &format!("{diff}{diff}")).is_err());
        assert!(discard_hunk(
            &git,
            &diff
                .replace("a/a.txt", "a/../a.txt")
                .replace("b/a.txt", "b/../a.txt")
        )
        .is_err());
        assert!(discard_hunk(
            &git,
            &diff.replace("diff --git", "diff --git\nnew file mode 100644\n")
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).expect("read"),
            "changed\n"
        );
    }

    #[test]
    fn leaves_the_file_alone_when_the_hunk_no_longer_applies() {
        let (dir, git) = repo();
        fs::write(dir.path().join("b.txt"), "edited\n").expect("edit");
        let diff = git.run_text(&["diff", "--", "b.txt"]).expect("diff");
        fs::write(dir.path().join("b.txt"), "something else\n").expect("edit");
        assert!(discard_hunk(&git, &diff).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("b.txt")).expect("read"),
            "something else\n"
        );
    }
}
