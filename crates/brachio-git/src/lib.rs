//! git operations for Brachio. Every write goes through the git CLI with an
//! argument vector.

mod branch_list;
mod change_ops;
mod changed_paths;
mod checkpoint;
mod conflict_report;
mod file_list;
mod git;
mod naming;
mod nul_records;
mod operation;
mod port_pool;
mod pull_request;
mod pull_request_status;
mod push_target;
mod repo_info;
mod worktree;
mod worktree_list;
mod worktree_setup;

pub use branch_list::{list_branches, BranchEntry, MAX_BRANCHES};
pub use change_ops::{discard_hunk, discard_tracked, in_head};
pub use changed_paths::{changed_paths, shared_paths};
pub use checkpoint::{
    capture_checkpoint, checkpoint_diff, delete_checkpoints, list_checkpoints, restore_checkpoint,
    CaptureKind, Checkpoint, CheckpointDiff, Restored, MAX_CHECKPOINTS,
};
pub use conflict_report::{merge_target, predict_conflict, ConflictReport, MergeTarget};
pub use file_list::{list_files, MAX_FILES};
pub use git::{Git, GitError};
pub use naming::{branch_name, slugify};
pub use operation::{stopped_operation, StoppedOperation};
pub use port_pool::{PortBlock, PortPool};
pub use pull_request::{create_pull_request, PullRequest, PullRequestError};
pub use pull_request_status::{
    backoff_seconds, fetch_pull_request, fetch_pull_request_by_number, local_head,
    parse_pull_request_reference, repository_of, ChecksOverall, ChecksSummary, Conflict,
    FailedCheck, PullRequestReference, PullRequestSnapshot, PullRequestState, ReviewState,
    StatusError,
};
pub use push_target::push_remote;
pub use repo_info::{find_repositories, inspect_repo, RepoInfo};
pub use worktree::{
    add_worktree, discard_worktree, remove_worktree, unsaved_work, worktree_path, AddWorktree,
    UnsavedWork,
};
pub use worktree_list::{
    importable, list_worktrees, parse_worktree_list, path_key, select_importable, ImportError,
    WorktreeEntry,
};
pub use worktree_setup::{
    copy_setup_files, read_setup, retry_setup_files, SetupConfig, SetupError, CONFIG_FILE,
};
