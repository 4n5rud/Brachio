//! SQLite storage. One writer connection in WAL mode.

mod database;
mod message_record;
mod pr_status_record;
mod repo_record;
mod task_record;
mod task_tabs;

pub use database::{open, open_in_memory, StoreError};
pub use message_record::{
    forget_task_messages, peek_unread, recent_messages, send_message, take_unread, unread_counts,
    Message, NewMessage, KINDS, MAX_BODY_BYTES,
};
pub use pr_status_record::{load_pr_status, save_pr_status};
pub use repo_record::{NewRepo, RepoRecord};
pub use task_record::{NewTask, TaskRecord};
pub use task_tabs::{load_secondary_tabs, set_secondary_tab};
