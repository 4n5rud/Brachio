//! git operations for Brachio. Every write goes through the git CLI with an
//! argument vector.

mod cli;
mod conflict;
mod naming;
mod ports;
mod worktree;

pub use cli::{Git, GitError};
pub use conflict::{predict_conflict, ConflictReport};
pub use naming::{branch_name, slugify};
pub use ports::{PortBlock, PortPool};
pub use worktree::{add_worktree, remove_worktree, AddWorktree};
