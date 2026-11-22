//! Runs the real git CLI to check that restore points leave the user's repository alone.

use std::fs;
use std::path::Path;

use brachio_git::{
    capture_checkpoint, checkpoint_diff, delete_checkpoints, list_checkpoints, restore_checkpoint,
    CaptureKind, Checkpoint, Git,
};

fn run(git: &Git, args: &[&str]) -> String {
    git.run(args)
        .unwrap_or_else(|error| panic!("git {args:?} failed: {error}"))
}

fn init_repo(dir: &Path) -> Git {
    let git = Git::new(dir);
    run(&git, &["init", "-q", "-b", "main"]);
    run(&git, &["config", "user.email", "test@example.com"]);
    run(&git, &["config", "user.name", "Brachio Test"]);
    run(&git, &["config", "core.autocrlf", "false"]);
    fs::write(dir.join(".gitignore"), "ignored.log\n").expect("write .gitignore");
    fs::write(dir.join("a.txt"), "base\n").expect("write a.txt");
    run(&git, &["add", "."]);
    run(&git, &["commit", "-q", "-m", "base"]);
    git
}

fn files_in(git: &Git, commit: &str) -> Vec<String> {
    run(git, &["ls-tree", "-r", "--name-only", commit])
        .lines()
        .map(str::to_owned)
        .collect()
}

fn capture(git: &Git, kind: CaptureKind) -> Option<Checkpoint> {
    capture_checkpoint(git, "task-1", kind).expect("capture")
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).expect("read")
}

#[test]
fn capture_keeps_untracked_files_and_skips_ignored_ones() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git = init_repo(dir.path());
    fs::write(dir.path().join("a.txt"), "changed\n").expect("write");
    fs::write(dir.path().join("new.txt"), "new\n").expect("write");
    fs::create_dir(dir.path().join("sub")).expect("mkdir");
    fs::write(dir.path().join("sub").join("deep.txt"), "deep\n").expect("write");
    fs::write(dir.path().join("ignored.log"), "noise\n").expect("write");

    let point = capture(&git, CaptureKind::TurnEnd).expect("created");

    assert_eq!(point.number, 1);
    assert_eq!(
        files_in(&git, &point.commit),
        [".gitignore", "a.txt", "new.txt", "sub/deep.txt"]
    );
    assert_eq!(
        run(&git, &["show", &format!("{}:a.txt", point.commit)]),
        "changed"
    );
}

#[test]
fn capture_is_skipped_when_nothing_changed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git = init_repo(dir.path());
    assert!(capture(&git, CaptureKind::TurnStart).is_some());
    assert!(capture(&git, CaptureKind::TurnEnd).is_none());

    fs::write(dir.path().join("a.txt"), "edited\n").expect("write");
    let second = capture(&git, CaptureKind::TurnEnd).expect("created");

    assert_eq!(second.number, 2);
    assert_eq!(
        (second.files, second.additions, second.deletions),
        (1, 1, 1)
    );
    assert_eq!(list_checkpoints(&git, "task-1").expect("list").len(), 2);
}

#[test]
fn capture_leaves_head_index_and_branch_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git = init_repo(dir.path());
    fs::write(dir.path().join("a.txt"), "staged\n").expect("write");
    run(&git, &["add", "a.txt"]);
    fs::write(dir.path().join("a.txt"), "staged then edited\n").expect("write");
    fs::write(dir.path().join("untracked.txt"), "u\n").expect("write");
    let git_dir = dir.path().join(".git");
    let index_before = fs::read(git_dir.join("index")).expect("index");
    let state = || {
        (
            run(&git, &["rev-parse", "HEAD"]),
            run(&git, &["status", "--porcelain=v1"]),
            run(&git, &["ls-files", "-s"]),
            run(&git, &["branch", "--list"]),
        )
    };
    let before = state();

    capture(&git, CaptureKind::TurnEnd).expect("created");

    assert_eq!(before, state());
    assert_eq!(
        index_before,
        fs::read(git_dir.join("index")).expect("index")
    );
    let leftovers = fs::read_dir(&git_dir)
        .expect("read .git")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("brachio-checkpoint")
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn restore_rewrites_files_and_can_itself_be_undone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    let git = init_repo(root);
    fs::write(root.join("a.txt"), "turn one\n").expect("write");
    fs::write(root.join("kept.txt"), "kept\n").expect("write");
    let first = capture(&git, CaptureKind::TurnEnd).expect("created");

    fs::write(root.join("a.txt"), "turn two\n").expect("write");
    fs::remove_file(root.join("kept.txt")).expect("remove");
    fs::create_dir_all(root.join("made").join("inner")).expect("mkdir");
    fs::write(root.join("made").join("inner").join("x.txt"), "x\n").expect("write");
    fs::write(root.join("ignored.log"), "still here\n").expect("write");
    let head = run(&git, &["rev-parse", "HEAD"]);

    let restored = restore_checkpoint(&git, root, "task-1", first.number).expect("restore");

    assert_eq!(read(root, "a.txt"), "turn one\n");
    assert_eq!(read(root, "kept.txt"), "kept\n");
    assert!(!root.join("made").exists());
    assert!(root.join("ignored.log").exists());
    assert_eq!(run(&git, &["rev-parse", "HEAD"]), head);
    assert_eq!(restored.backup_number, 2);

    restore_checkpoint(&git, root, "task-1", restored.backup_number).expect("undo");

    assert_eq!(read(root, "a.txt"), "turn two\n");
    assert!(!root.join("kept.txt").exists());
    assert_eq!(read(root, "made/inner/x.txt"), "x\n");
}

#[test]
fn diff_compares_a_point_with_the_one_before() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git = init_repo(dir.path());
    let first = capture(&git, CaptureKind::TurnStart).expect("created");
    fs::write(dir.path().join("a.txt"), "base\nmore\n").expect("write");
    let second = capture(&git, CaptureKind::TurnEnd).expect("created");

    let diff = checkpoint_diff(&git, "task-1", second.number).expect("diff");

    assert_eq!(diff.base_number, Some(first.number));
    assert!(diff.patch.contains("+more"));
    let oldest = checkpoint_diff(&git, "task-1", first.number).expect("first");
    assert!(oldest.patch.is_empty());
}

#[test]
fn deleting_a_task_removes_only_its_restore_points() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git = init_repo(dir.path());
    fs::write(dir.path().join("a.txt"), "one\n").expect("write");
    capture_checkpoint(&git, "task-1", CaptureKind::TurnEnd).expect("capture");
    capture_checkpoint(&git, "task-2", CaptureKind::TurnEnd).expect("capture");

    delete_checkpoints(&git, "task-1").expect("delete");

    assert!(list_checkpoints(&git, "task-1").expect("list").is_empty());
    assert_eq!(list_checkpoints(&git, "task-2").expect("list").len(), 1);
}
