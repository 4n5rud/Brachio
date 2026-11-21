//! Runs the real git CLI against a throwaway repository.

use std::fs;
use std::path::{Path, PathBuf};

use brachio_git::{AddWorktree, Git, GitError};

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
    let error = brachio_git::inspect_repo(folder.path()).expect_err("a folder without .git");
    assert!(
        matches!(&error, GitError::NotARepository { suggestions, .. } if suggestions.is_empty()),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains("Choose the folder that contains .git"),
        "{error}"
    );
}

#[test]
fn inspect_points_at_the_repository_inside_the_chosen_folder() {
    let parent = temp_dir();
    let repo = parent.path().join("project");
    fs::create_dir(&repo).expect("make project folder");
    init_repo(&repo);
    fs::create_dir(parent.path().join("notes")).expect("make a plain sibling folder");

    let error = brachio_git::inspect_repo(parent.path()).expect_err("the parent is not a repo");
    match &error {
        GitError::NotARepository { suggestions, .. } => {
            assert_eq!(suggestions, std::slice::from_ref(&repo))
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(error.to_string().contains("Did you mean"), "{error}");
    assert!(error.to_string().contains("project"), "{error}");
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
    assert_eq!(work.uncommitted_files, 1);
    assert_eq!(work.unpushed_commits, 1);
    assert_eq!(work.ignored_entries, 0);
    assert!(work.ignored_examples.is_empty());

    brachio_git::discard_worktree(&git, &tree, "brachio/d-4").expect("discard");
    assert!(!tree.exists(), "the worktree folder must be gone");
    let branches = git
        .run(&["branch", "--list", "brachio/d-4"])
        .expect("list branches");
    assert!(branches.is_empty(), "the branch must be gone: {branches}");
}

#[test]
fn the_fingerprint_changes_when_work_appears_after_the_preview() {
    let repo = temp_dir();
    let trees = temp_dir();
    let git = init_repo(repo.path());
    fs::write(repo.path().join(".gitignore"), "*.log\nbuild/\n").expect("write ignore");
    run(&git, &["add", ".gitignore"]);
    run(&git, &["commit", "-q", "-m", "ignore logs"]);
    let tree = trees.path().join("f");
    add(&git, "brachio/f-6", &tree);
    let fingerprint = || {
        brachio_git::unsaved_work(&git, &tree, "brachio/f-6")
            .expect("preview")
            .fingerprint
    };

    let first = fingerprint();
    assert_eq!(
        first,
        fingerprint(),
        "unchanged state must keep its fingerprint"
    );

    fs::write(tree.join("note.txt"), "new\n").expect("write untracked");
    let untracked = fingerprint();
    assert_ne!(first, untracked);

    fs::write(tree.join("note.txt"), "NEW\n").expect("edit untracked in place");
    let edited_untracked = fingerprint();
    assert_ne!(untracked, edited_untracked);

    fs::write(tree.join("run.log"), "log\n").expect("write ignored");
    let ignored = fingerprint();
    assert_ne!(edited_untracked, ignored);
    fs::write(tree.join("run.log"), "LOG\n").expect("edit ignored in place");
    let edited_ignored = fingerprint();
    assert_ne!(ignored, edited_ignored);

    fs::create_dir(tree.join("build")).expect("create ignored directory");
    fs::write(tree.join("build/out"), "one").expect("write ignored output");
    let ignored_directory = fingerprint();
    fs::write(tree.join("build/out"), "two").expect("edit ignored output in place");
    let edited_directory = fingerprint();
    assert_ne!(ignored_directory, edited_directory);
    fs::write(tree.join("build/another"), "new").expect("add ignored output");
    let expanded_directory = fingerprint();
    assert_ne!(edited_directory, expanded_directory);

    fs::write(tree.join("a.txt"), "edited\n").expect("edit tracked");
    let modified = fingerprint();
    assert_ne!(expanded_directory, modified);
    fs::write(tree.join("a.txt"), "edited again\n").expect("edit tracked again");
    assert_ne!(
        modified,
        fingerprint(),
        "more edits to a changed file must show"
    );

    commit_in(&tree, "committed\n");
    assert_ne!(modified, fingerprint());
}

#[test]
fn untracked_contents_over_the_scan_limit_block_delete_confirmation() {
    let repo = temp_dir();
    let trees = temp_dir();
    let git = init_repo(repo.path());
    let tree = trees.path().join("large");
    let branch = "brachio/large";
    add(&git, branch, &tree);
    fs::File::create(tree.join("large-output"))
        .expect("create large untracked output")
        .set_len(257 * 1024 * 1024)
        .expect("resize output");

    let error = brachio_git::unsaved_work(&git, &tree, branch).expect_err("scan must be bounded");
    assert!(error.to_string().contains("exceed the scan limits"));
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

#[test]
fn inspect_does_not_adopt_a_checked_out_branch_when_a_remote_names_no_default() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    run(&git, &["branch", "-m", "feature-x"]);
    run(
        &git,
        &["remote", "add", "origin", "https://example.invalid/r.git"],
    );

    let error = brachio_git::inspect_repo(repo.path()).expect_err("no way to tell");
    assert!(matches!(error, GitError::NoDefaultBranch { .. }), "{error}");
}

#[test]
fn inspect_uses_the_checked_out_branch_in_a_repository_without_remotes() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    run(&git, &["branch", "-m", "feature-x"]);

    let info = brachio_git::inspect_repo(repo.path()).expect("inspect");
    assert_eq!(info.default_branch, "feature-x");
}

#[test]
fn a_worktree_can_be_made_for_a_korean_task_title() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    let first = brachio_git::branch_name("로그인 버그 수정", "a1b2c3");
    let second = brachio_git::branch_name("결제 오류", "d4e5f6");
    assert_ne!(first, second);

    let trees = temp_dir();
    let path = trees.path().join(first.rsplit('/').next().expect("leaf"));
    add(&git, &first, &path);

    assert!(path.join("a.txt").exists());
    assert_eq!(
        Git::new(&path)
            .run(&["branch", "--show-current"])
            .expect("current branch"),
        first
    );
}

#[test]
fn push_goes_to_the_only_remote_even_when_it_is_not_called_origin() {
    let repo = temp_dir();
    let remote = temp_dir();
    let git = init_repo(repo.path());
    run(&Git::new(remote.path()), &["init", "-q", "--bare"]);
    run(
        &git,
        &["remote", "add", "backup", &remote.path().to_string_lossy()],
    );

    assert_eq!(
        brachio_git::push_remote(&git, "main").expect("pick remote"),
        "backup"
    );

    run(
        &git,
        &["remote", "add", "mirror", &remote.path().to_string_lossy()],
    );
    assert!(brachio_git::push_remote(&git, "main").is_err());
    run(&git, &["config", "branch.main.pushRemote", "mirror"]);
    assert_eq!(
        brachio_git::push_remote(&git, "main").expect("configured remote wins"),
        "mirror"
    );
}

#[test]
fn deleting_a_task_counts_ignored_files_that_no_commit_holds() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    fs::write(repo.path().join(".gitignore"), ".env.local\nbuild/\n").expect("write ignore");
    run(&git, &["add", ".gitignore"]);
    run(&git, &["commit", "-q", "-m", "ignore local files"]);
    let trees = temp_dir();
    let tree = trees.path().join("t");
    add(&git, "brachio/t-1", &tree);

    let clean = brachio_git::unsaved_work(&git, &tree, "brachio/t-1").expect("preview");
    assert_eq!(clean.ignored_entries, 0);

    fs::write(tree.join(".env.local"), "SECRET=1\n").expect("write env");
    fs::create_dir(tree.join("build")).expect("make build");
    fs::write(tree.join("build").join("out.bin"), "x").expect("write output");
    let work = brachio_git::unsaved_work(&git, &tree, "brachio/t-1").expect("preview");
    assert_eq!(work.uncommitted_files, 0, "ignored files are not changes");
    assert_eq!(work.ignored_entries, 2);
    assert!(work.ignored_examples.contains(&".env.local".to_owned()));
    assert!(work.ignored_examples.contains(&"build/".to_owned()));
}

#[test]
fn a_huge_ignored_listing_is_counted_exactly_and_only_the_first_names_are_kept() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    fs::write(repo.path().join(".gitignore"), "*.tmp\n").expect("write ignore");
    run(&git, &["add", ".gitignore"]);
    run(&git, &["commit", "-q", "-m", "ignore temp files"]);
    let trees = temp_dir();
    let tree = trees.path().join("h");
    add(&git, "brachio/h-1", &tree);

    // Long names push the listing past several stream chunks.
    let stem = "generated-한글-".repeat(8);
    for index in 0..3000 {
        fs::write(tree.join(format!("{stem}{index:05}.tmp")), "x").expect("write ignored");
    }
    let work = brachio_git::unsaved_work(&git, &tree, "brachio/h-1").expect("preview");
    assert_eq!(work.ignored_entries, 3000);
    assert_eq!(work.uncommitted_files, 0);
    assert_eq!(work.ignored_examples.len(), 5);
    assert!(work
        .ignored_examples
        .iter()
        .all(|name| name.starts_with(&stem) && name.ends_with(".tmp")));
}

#[test]
fn a_rename_is_one_changed_file_and_a_changed_name_changes_the_fingerprint() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    let trees = temp_dir();
    let tree = trees.path().join("r");
    add(&git, "brachio/r-1", &tree);
    let tree_git = Git::new(&tree);
    run(&tree_git, &["mv", "a.txt", "renamed 한글.txt"]);
    fs::write(tree.join("other.txt"), "new\n").expect("write untracked");

    let first = brachio_git::unsaved_work(&git, &tree, "brachio/r-1").expect("preview");
    assert_eq!(first.uncommitted_files, 2);

    run(&tree_git, &["mv", "renamed 한글.txt", "renamed again.txt"]);
    let second = brachio_git::unsaved_work(&git, &tree, "brachio/r-1").expect("preview");
    assert_eq!(second.uncommitted_files, 2);
    assert_ne!(first.fingerprint, second.fingerprint);
}

#[test]
fn the_merge_target_is_the_remote_copy_of_the_default_branch_when_the_local_one_is_behind() {
    let repo = temp_dir();
    let git = init_repo(repo.path());
    let local = brachio_git::merge_target(&git, "main");
    assert_eq!(local.reference, "refs/heads/main");
    assert!(
        !local.remote_tracking,
        "without a remote the local branch is used and says so"
    );

    let remote = temp_dir();
    run(&Git::new(remote.path()), &["init", "-q", "--bare"]);
    run(
        &git,
        &["remote", "add", "origin", &remote.path().to_string_lossy()],
    );
    run(&git, &["push", "-q", "origin", "main"]);
    run(&git, &["fetch", "-q", "origin"]);
    let tracked = brachio_git::merge_target(&git, "main");
    assert_eq!(tracked.reference, "refs/remotes/origin/main");
    assert!(tracked.remote_tracking);
}

#[test]
fn repositories_inside_a_folder_are_found_two_levels_down_and_their_own_folders_are_not_searched() {
    let folder = temp_dir();
    for repo in [
        "one",
        "group/two",
        "group/deep/three",
        "node_modules/pkg",
        ".hidden/four",
    ] {
        let path = folder.path().join(repo);
        fs::create_dir_all(path.join(".git")).expect("make repo marker");
    }
    fs::create_dir_all(folder.path().join("plain")).expect("make plain folder");
    fs::create_dir_all(folder.path().join("one/inner/.git")).expect("make nested repo");

    let found: Vec<String> = brachio_git::find_repositories(folder.path())
        .iter()
        .map(|path| {
            path.strip_prefix(folder.path())
                .expect("inside the folder")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(found, ["group/two", "one"]);
}

#[test]
fn worktrees_made_outside_brachio_are_listed_and_importable() {
    let repo = temp_dir();
    let trees = temp_dir();
    let git = init_repo(repo.path());
    let outside = trees.path().join("hotfix");
    add(&git, "fix/crash", &outside);
    let entries = brachio_git::list_worktrees(&git).expect("list worktrees");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].branch.as_deref(), Some("main"));
    assert_eq!(entries[1].branch.as_deref(), Some("fix/crash"));
    let found = brachio_git::importable(&entries, &[repo.path().to_path_buf()]);
    assert_eq!(found.len(), 1);
    assert_eq!(
        brachio_git::path_key(&found[0].path),
        brachio_git::path_key(&outside)
    );
    assert!(brachio_git::importable(&entries, &[outside]).is_empty());
}
