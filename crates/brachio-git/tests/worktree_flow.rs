//! Runs the real git CLI against a throwaway repository.
#![allow(clippy::unwrap_used)] // Test helpers outside #[test] functions.

use std::fs;
use std::path::Path;

use brachio_git::{add_worktree, predict_conflict, remove_worktree, AddWorktree, Git};

fn init_repo(dir: &Path) -> Git {
    let git = Git::new(dir);
    git.run(&["init", "-q", "-b", "main"]).unwrap();
    git.run(&["config", "user.email", "test@example.com"])
        .unwrap();
    git.run(&["config", "user.name", "Brachio Test"]).unwrap();
    fs::write(dir.join("a.txt"), "base\n").unwrap();
    git.run(&["add", "a.txt"]).unwrap();
    git.run(&["commit", "-q", "-m", "base"]).unwrap();
    git
}

fn commit_in(dir: &Path, content: &str) {
    let git = Git::new(dir);
    fs::write(dir.join("a.txt"), content).unwrap();
    git.run(&["commit", "-q", "-am", content.trim()]).unwrap();
}

#[test]
fn parallel_worktrees_predict_conflict_then_clean_up() {
    let repo = tempfile::tempdir().unwrap();
    let trees = tempfile::tempdir().unwrap();
    let git = init_repo(repo.path());

    let wt_a = trees.path().join("a");
    let wt_b = trees.path().join("b");
    for (branch, path) in [("brachio/a-1", &wt_a), ("brachio/b-2", &wt_b)] {
        let req = AddWorktree {
            branch: branch.into(),
            path: path.clone(),
            base: "main".into(),
        };
        add_worktree(&git, &req).unwrap();
    }
    commit_in(&wt_a, "from a\n");
    commit_in(&wt_b, "from b\n");

    let report = predict_conflict(&git, "brachio/a-1", "brachio/b-2").unwrap();
    assert_eq!(report.conflicted_files, vec!["a.txt"]);

    // Unmerged branch: `branch -d` must refuse, so the work is not lost.
    assert!(remove_worktree(&git, &wt_a, "brachio/a-1").is_err());
}

#[test]
fn disjoint_changes_are_clean() {
    let repo = tempfile::tempdir().unwrap();
    let trees = tempfile::tempdir().unwrap();
    let git = init_repo(repo.path());
    let wt = trees.path().join("c");
    let req = AddWorktree {
        branch: "brachio/c-3".into(),
        path: wt.clone(),
        base: "main".into(),
    };
    add_worktree(&git, &req).unwrap();
    fs::write(wt.join("b.txt"), "new\n").unwrap();
    Git::new(&wt).run(&["add", "b.txt"]).unwrap();
    Git::new(&wt).run(&["commit", "-q", "-m", "add b"]).unwrap();

    assert!(predict_conflict(&git, "main", "brachio/c-3")
        .unwrap()
        .is_clean());
}
