//! Runs the real git CLI against a throwaway repository.

use std::fs;
use std::path::{Path, PathBuf};

use brachio_git::{AddWorktree, Git};

fn run(git: &Git, args: &[&str]) {
    if let Err(error) = git.run(args) {
        panic!("git {args:?} failed: {error}");
    }
}

fn init_repo(dir: &Path) -> Git {
    let git = Git::new(dir);
    run(&git, &["init", "-q", "-b", "main"]);
    run(&git, &["config", "user.email", "test@example.com"]);
    run(&git, &["config", "user.name", "Brachio Test"]);
    fs::write(dir.join("a.txt"), "base\n").expect("write a.txt");
    run(&git, &["add", "a.txt"]);
    run(&git, &["commit", "-q", "-m", "base"]);
    git
}

fn commit_in(dir: &Path, content: &str) {
    fs::write(dir.join("a.txt"), content).expect("write a.txt");
    run(&Git::new(dir), &["commit", "-q", "-am", content.trim()]);
}

fn add(git: &Git, branch: &str, path: &Path) {
    let request = AddWorktree {
        branch: branch.to_owned(),
        path: path.to_path_buf(),
        base: "main".to_owned(),
    };
    brachio_git::add_worktree(git, &request).expect("add worktree");
}

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create temp dir")
}

#[test]
fn parallel_worktrees_predict_conflict_then_clean_up() {
    let repo = temp_dir();
    let trees = temp_dir();
    let git = init_repo(repo.path());
    let tree_a: PathBuf = trees.path().join("a");
    let tree_b: PathBuf = trees.path().join("b");
    add(&git, "brachio/a-1", &tree_a);
    add(&git, "brachio/b-2", &tree_b);
    commit_in(&tree_a, "from a\n");
    commit_in(&tree_b, "from b\n");

    let report =
        brachio_git::predict_conflict(&git, "brachio/a-1", "brachio/b-2").expect("merge-tree runs");
    assert_eq!(report.conflicted_files, vec!["a.txt"]);

    let removed = brachio_git::remove_worktree(&git, &tree_a, "brachio/a-1");
    assert!(removed.is_err(), "an unmerged branch must not be deleted");
}

#[test]
fn disjoint_changes_are_clean() {
    let repo = temp_dir();
    let trees = temp_dir();
    let git = init_repo(repo.path());
    let tree = trees.path().join("c");
    add(&git, "brachio/c-3", &tree);
    fs::write(tree.join("b.txt"), "new\n").expect("write b.txt");
    run(&Git::new(&tree), &["add", "b.txt"]);
    run(&Git::new(&tree), &["commit", "-q", "-m", "add b"]);

    let report =
        brachio_git::predict_conflict(&git, "main", "brachio/c-3").expect("merge-tree runs");
    assert!(
        report.is_clean(),
        "changes to different files must not conflict: {report:?}"
    );
}
