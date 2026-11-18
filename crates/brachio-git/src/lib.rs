//! git operations for Brachio. Every write goes through the git CLI with an
//! argument vector.

mod conflict_report;
mod git;
mod naming;
mod port_pool;
mod pull_request;
mod repo_info;
mod worktree;

pub use conflict_report::{predict_conflict, ConflictReport};
pub use git::{Git, GitError};
pub use naming::{branch_name, slugify};
pub use port_pool::{PortBlock, PortPool};
pub use pull_request::{create_pull_request, PullRequest, PullRequestError};
pub use repo_info::{inspect_repo, RepoInfo};
pub use worktree::{
    add_worktree, discard_worktree, remove_worktree, unsaved_work, worktree_path, AddWorktree,
    UnsavedWork,
};
