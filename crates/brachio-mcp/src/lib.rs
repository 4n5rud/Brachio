//! Brachio MCP server (stdio). Lands in M3.
//! The tool names are fixed here so adapters and docs agree on them.

/// Starts a worker task in its own worktree.
pub const TOOL_SPAWN_TASK: &str = "spawn_task";
/// Runs the project's Playwright tests for a task.
pub const TOOL_RUN_QA: &str = "run_qa";
/// Reports the agent's own status to the board.
pub const TOOL_REPORT_STATUS: &str = "report_status";
/// Returns a worker task's result to the lead agent.
pub const TOOL_GET_TASK_RESULT: &str = "get_task_result";
/// Asks a person to review a task's changes.
pub const TOOL_REQUEST_REVIEW: &str = "request_review";
/// Records a finding from exploratory QA.
pub const TOOL_REPORT_QA_FINDING: &str = "report_qa_finding";
