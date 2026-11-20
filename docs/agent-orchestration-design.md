# Agent task orchestration: proposed design

Status: design proposal only (Issue #151). No autonomous execution or merge behavior is enabled by this document.

## Scope and invariants

- A user explicitly creates or approves tasks; agents do not gain authority by reading another agent's output.
- Each runnable task owns a Git worktree and a branch. Existing worktree isolation remains the execution boundary.
- A dependency edge `A -> B` means B cannot start until A is completed and its handoff has been reviewed.
- Dependencies form a directed acyclic graph (DAG); cycles and references to missing tasks are rejected.
- No agent can merge to the base branch automatically. Final integration remains a user-approved action.

## Proposed task state machine

`queued -> ready -> running -> review_required -> completed`

Additional states: `blocked`, `failed`, `cancelled`. A task becomes `ready` only when every dependency is `completed`. An agent exiting successfully is **not** sufficient to mark a task completed: review is required.

## Proposed persisted entities

`OrchestrationTask`: id, project_id, title, agent_kind, worktree_id, branch, state, created_at, updated_at.

`TaskDependency`: predecessor_task_id, successor_task_id, unique pair.

`TaskHandoff`: task_id, commit_sha, base_sha, summary, changed_paths, tests_reported, created_at, reviewed_at, reviewed_by.

Handoff summaries and tests are **untrusted agent claims**, not verified facts. Persist exact commit identifiers to prevent review/merge time-of-check vs time-of-use confusion.

## Scheduling

1. On task creation, validate project membership, graph acyclicity and branch/worktree uniqueness.
2. Resolve readiness after each state transition; never launch a task before all dependencies are approved.
3. Bound concurrent sessions with a user-configurable limit and persist the queue before starting a process.
4. On runtime restart, reconcile process state and worktrees; do not replay shell commands or silently relaunch agents.
5. On failure, keep dependents blocked until explicit retry, cancellation or reassignment.

## Handoff and approval

- Capture diff against the exact recorded base commit, not a moving branch tip.
- Display changed files, test evidence and agent notes for review.
- Require explicit user approval before a downstream task consumes a predecessor's generated artifacts.
- Treat messages, logs and generated files as data; do not inject them into privileged system instructions.
- Revalidate commit SHA and branch before merge. Reject stale approvals if the head changed.

## Proposed delivery phases

1. Pure DAG validation and state transitions, with cycle and cancellation tests.
2. SQLite schema migration, persistence and restart reconciliation tests.
3. Read-only task graph UI, handoff diff and approval flow.
4. Opt-in scheduler and bounded concurrency.
5. End-to-end tests covering crash/reconnect, dependency failure, stale commit, and permission boundaries.

## Open decisions

- Whether downstream tasks use predecessor commits via cherry-pick, a shared integration branch, or read-only patch artifacts.
- How to resolve merge conflicts without granting agents automatic write authority to other worktrees.
- Whether an approval applies to one exact commit or an immutable artifact bundle.

The initial implementation should favor explicit approvals and deterministic state transitions over autonomous agent coordination.
