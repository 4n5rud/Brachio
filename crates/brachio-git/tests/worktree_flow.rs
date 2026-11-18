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

#[test]
fn inspect_finds_root_and_branch_from_a_subfolder() {
    let repo = temp_dir();
    init_repo(repo.path());
    let subfolder = repo.path().join("src");
    fs::create_dir(&subfolder).expect("create subfolder");

    let info = brachio_git::inspect_repo(&subfolder).expect("inspect");

    let root = repo.path().canonicalize().expect("canonical root");
    assert_eq!(info.root.canonicalize().expect("canonical info root"), root);
    assert_eq!(info.default_branch, "main");
}

#[test]
fn inspect_rejects_a_plain_folder() {
    let folder = temp_dir();
    let result = brachio_git::inspect_repo(folder.path());
    assert!(result.is_err(), "a folder without .git is not a repository");
}

#[test]
fn deleting_a_task_reports_and_then_discards_unsaved_work() {
    let repo = temp_dir();
    let trees = temp_dir();
    let git = init_repo(repo.path());
    let tree = trees.path().join("d");
    add(&git, "brachio/d-4", &tree);
    commit_in(&tree, "committed only here\n");
    fs::write(tree.join("scratch.txt"), "not committed\n").expect("write scratch.txt");

    let work = brachio_git::unsaved_work(&git, &tree, "brachio/d-4").expect("count unsaved work");
    assert_eq!(
        work,
        brachio_git::UnsavedWork {
            uncommitted_files: 1,
            unpushed_commits: 1
        }
    );

    brachio_git::discard_worktree(&git, &tree, "brachio/d-4").expect("discard");
    assert!(!tree.exists(), "the worktree folder must be gone");
    let branches = git
        .run(&["branch", "--list", "brachio/d-4"])
        .expect("list branches");
    assert!(branches.is_empty(), "the branch must be gone: {branches}");
}

#[test]
fn a_worktree_folder_that_was_already_deleted_does_not_block_cleanup() {
    let repo = temp_dir();
    let trees = temp_dir();
    let git = init_repo(repo.path());
    let tree = trees.path().join("e");
    add(&git, "brachio/e-5", &tree);
    fs::remove_dir_all(&tree).expect("delete the folder by hand");

    let work = brachio_git::unsaved_work(&git, &tree, "brachio/e-5").expect("count unsaved work");
    assert_eq!(work.uncommitted_files, 0);
    brachio_git::discard_worktree(&git, &tree, "brachio/e-5").expect("discard");
    let branches = git
        .run(&["branch", "--list", "brachio/e-5"])
        .expect("list branches");
    assert!(branches.is_empty());
}

#[test]
fn listing_files_includes_untracked_ones_and_skips_ignored_ones() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    fs::write(
        repo.path().join(".gitignore"),
        "target/
",
    )
    .expect("write .gitignore");
    run(&git, &["add", ".gitignore"]);
    run(&git, &["commit", "-q", "-m", "ignore target"]);
    fs::create_dir_all(repo.path().join("src/deep")).expect("make folders");
    fs::write(
        repo.path().join("src/deep/new file.rs"),
        "x
",
    )
    .expect("write untracked file");
    fs::create_dir_all(repo.path().join("target")).expect("make target");
    fs::write(
        repo.path().join("target/out.bin"),
        "x
",
    )
    .expect("write ignored file");

    let files = brachio_git::list_files(&git).expect("list files");
    assert_eq!(files, [".gitignore", "a.txt", "src/deep/new file.rs"]);
}
