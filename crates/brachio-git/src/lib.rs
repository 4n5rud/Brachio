//! git operations for Brachio. Every write goes through the git CLI with an
//! argument vector.

mod conflict_report;
mod file_list;
mod git;
mod naming;
mod port_pool;
mod pull_request;
mod repo_info;
mod worktree;
mod worktree_setup;

pub use conflict_report::{predict_conflict, ConflictReport};
pub use file_list::{list_files, MAX_FILES};
pub use git::{Git, GitError};
pub use naming::{branch_name, slugify};
pub use port_pool::{PortBlock, PortPool};
pub use pull_request::{create_pull_request, PullRequest, PullRequestError};
pub use repo_info::{inspect_repo, RepoInfo};
pub use worktree::{
    add_worktree, discard_worktree, remove_worktree, unsaved_work, worktree_path, AddWorktree,
    UnsavedWork,
};
pub use worktree_setup::{
    copy_setup_files, read_setup, retry_setup_files, SetupConfig, SetupError, CONFIG_FILE,
};
