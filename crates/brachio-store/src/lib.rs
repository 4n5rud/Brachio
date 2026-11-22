//! SQLite storage. One writer connection in WAL mode.

mod child_report;
mod database;
mod decision_gate_record;
mod message_record;
mod orchestration_record;
mod orchestration_schedule;
mod pr_status_record;
mod project_cue_record;
mod prompt_history_record;
mod qa_result_record;
mod repo_record;
mod task_activity_record;
mod task_pin_record;
mod task_record;
mod task_tabs;
mod usage_record;

pub use child_report::{child_reports, ChildReport};
pub use database::{open, open_in_memory, StoreError};
pub use decision_gate_record::{
    gate_options, pending_gate, pending_gates, PendingGate, MAX_GATE_OPTIONS, MAX_GATE_OPTION_CHARS,
};
pub use message_record::{
    dismiss_messages, expire_read_messages, forget_task_messages, mark_read, peek_inbox,
    peek_next_batch, peek_unread, recent_messages, reply_recipient, send_message, take_unread,
    unread_counts, Message, NewMessage, KINDS, MAX_BODY_BYTES, MAX_PAYLOAD_BYTES,
    MAX_SUBJECT_CHARS, MAX_UNREAD_FROM_ONE_SENDER,
};
pub use orchestration_record::{
    add_dependency, apply_transition, decide_handoff, decide_handoff_then, load_graph,
    mark_running, record_handoff, review_handoff, submit_handoff, Handoff, HandoffReview,
    HandoffSource, NewHandoff, OrchestrationError,
};
pub use orchestration_schedule::{
    fail_lost, fail_running, record_start_failure, reopen_failed, running_children, schedule_next,
    scheduled_leads, scheduler_enabled, set_scheduler_enabled, start_circuit_broken, state_reason,
    StartOutcome, LOST_AFTER_RESTART, MAX_RUNNING_CHILDREN, MAX_START_FAILURES,
};
pub use pr_status_record::{forget_pr_status, load_pr_status, save_pr_status};
pub use project_cue_record::{
    add_project_cue, delete_project_cue, project_cues, ProjectCue, MAX_CUE_BODY_CHARS,
    MAX_CUE_LABEL_CHARS,
};
pub use prompt_history_record::{
    add_prompt_history, prompt_history, PromptHistoryEntry, MAX_PROMPT_HISTORY,
};
pub use qa_result_record::{
    delete_qa_results, latest_qa_result, qa_result_by_id, save_qa_result, NewQaResult,
    QaResultRecord, MAX_QA_RESULTS_PER_TASK,
};
pub use repo_record::{NewRepo, RepoRecord};
pub use task_activity_record::{
    record_activity, task_activity, ActivityEntry, ACTIVITY_KINDS, MAX_ACTIVITY_DETAIL_CHARS,
    MAX_TASK_ACTIVITY,
};
pub use task_pin_record::{pinned_task_ids, set_task_pinned};
pub use task_record::{ChildInsertError, NewTask, TaskRecord};
pub use task_tabs::{
    add_task_tab, close_task_tab, forget_retired_secondary_tab, load_task_tabs, rename_task_tab,
    reorder_task_tabs, retired_secondary_tabs, task_tab, task_tab_names, task_tabs, TabKind,
    TaskTab,
};
pub use usage_record::{
    limit_history, record_limit_snapshot, usage_apply, usage_file_state, usage_reset, usage_rows,
    LimitSnapshot, UsageRow, SNAPSHOT_GAP_MS,
};
