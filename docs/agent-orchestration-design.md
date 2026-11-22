# ADR: Lead and worker task orchestration

Status: partly implemented. The sections below mark what ships today and what is still planned.

## Background

Users run several coding agents at once. Splitting a large piece of work across agents helps only if each agent works in isolation, the order of dependent work is respected, and a person stays in charge of what reaches the base branch. Agents' own output is not trustworthy enough to drive that decision.

## Decision

- A **lead** task, started by the user, can ask for **child** tasks. Each child has its own Git worktree, branch and agent.
- A child may wait for a sibling (`--after`). It starts only after that sibling is completed and its handoff has been reviewed by the user.
- A finished child reports a **handoff**: its commit, the changed files and a summary. The summary and any test claims are the agent's own statements, not verified facts. The recorded commit identifies exactly what was reviewed.
- Messages between tasks are information. They never grant permission to approve, merge, push or delete; only the user can.
- Nothing merges automatically. Integration stays a user-approved action.
- Dependencies form a directed acyclic graph. Cycles and references to missing tasks are rejected.

## Implemented now

- Lead and child tasks created with `brachio task create`, with `--after` dependencies and one level of nesting. A task has at most 4 open children.
- Child tasks in Git projects only. Plain folders do not support them.
- Approval of new child tasks in the app, and review of each child's handoff before dependent tasks start.
- Task messaging with `brachio send`, `ask`, `reply` and `check`.
- Setup commands and QA runs gated by approval.

## Rejected alternatives

- **Agents merge each other's work automatically.** Rejected: it hands write authority across worktrees to an agent and removes the human review point.
- **One shared worktree for all agents.** Rejected: simultaneous edits to the same files collide and are hard to attribute.
- **Treating a successful exit as completion.** Rejected: an agent exiting says nothing about whether the work is correct, so review is required.
- **Replaying commands or silently relaunching agents after a restart.** Rejected: it can repeat side effects. Restarts reconcile state and ask the user.

## Future work

- Automatic merging, if ever, only behind an explicit user approval tied to the exact reviewed commit (stale approvals rejected when the head changes).
- Nesting deeper than one level, and child tasks in plain folders.
- A task graph view and a handoff diff view with its QA result.
- A user-configurable limit on concurrent sessions.
- How downstream tasks consume a predecessor's work: cherry-pick, a shared integration branch, or read-only patch artifacts.
- Resolving merge conflicts without giving agents write access to other worktrees.
