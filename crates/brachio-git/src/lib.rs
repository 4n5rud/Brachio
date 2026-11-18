//! git operations for Brachio. Every write goes through the git CLI with an
//! argument vector.

mod conflict_report;
mod git;
mod naming;
mod port_pool;
mod worktree;

pub use conflict_report::{predict_conflict, ConflictReport};
pub use git::{Git, GitError};
pub use naming::{branch_name, slugify};
pub use port_pool::{PortBlock, PortPool};
pub use worktree::{add_worktree, remove_worktree, worktree_path, AddWorktree};
