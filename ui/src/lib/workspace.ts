import { invoke } from "@tauri-apps/api/core";

import type { AgentKind as CoreAgent } from "./terminal";
import type { AgentKind, TaskRow } from "./tasks";

/** A repository as the core stores it. */
export interface RepoRecord {
  id: string;
  name: string;
  path: string;
  defaultBranch: string;
  addedAtMs: number;
}

/** A task as the core stores it. */
export interface TaskRecord {
  id: string;
  repoId: string;
  title: string;
  branch: string;
  worktreePath: string;
  agentKind: CoreAgent | null;
  createdAtMs: number;
  archivedAtMs: number | null;
  /** Conversation id the agent resumes, when Brachio picked one. */
  agentSessionId: string | null;
}

export interface CreateTaskRequest {
  repoId: string;
  title: string;
  agent: CoreAgent;
  /** `null` branches from the repository's default branch. */
  base: string | null;
}

export const listRepos = () => invoke<RepoRecord[]>("repo_list");
export const addRepo = (path: string) => invoke<RepoRecord>("repo_add", { path });
export const listTasks = () => invoke<TaskRecord[]>("task_list");
export const createTask = (request: CreateTaskRequest) => invoke<TaskRecord>("task_create", { request });
export const archiveTask = (id: string) => invoke<void>("task_archive", { id });
export const listArchivedTasks = () => invoke<TaskRecord[]>("task_list_archived");
export const restoreTask = (id: string) => invoke<TaskRecord>("task_restore", { id });

/** What deleting an archived task would destroy. */
export interface TaskLoss {
  uncommittedFiles: number;
  unpushedCommits: number;
}
export const previewTaskDeletion = (id: string) => invoke<TaskLoss>("task_delete_preview", { id });
export const deleteTask = (id: string) => invoke<void>("task_delete", { id });

interface TaskChanges {
  patch: string;
  untracked: string[];
  staged: string[];
  truncated: boolean;
}

export const readTaskChanges = (id: string) => invoke<TaskChanges>("task_changes", { id });
export type TaskGitAction =
  | { action: "stageTracked" | "push" }
  | { action: "stageFile" | "unstageFile"; path: string }
  | { action: "commit"; message: string };
export const runTaskGitAction = (id: string, request: TaskGitAction) =>
  invoke<void>("task_git_action", { id, request });

const UI_AGENT: Record<CoreAgent, AgentKind> = { claude_code: "claude", codex: "codex" };

/**
 * Shapes a stored task for the list. Live status and diff counts are not
 * tracked yet, so every task starts as "checking" with no changes.
 */
export function toTaskRow(record: TaskRecord): TaskRow {
  return {
    id: record.id,
    repoId: record.repoId,
    title: record.title,
    agent: UI_AGENT[record.agentKind ?? "claude_code"],
    branch: record.branch,
    status: { state: "unknown" },
    additions: 0,
    deletions: 0,
    updatedAt: record.createdAtMs,
    worktreePath: record.worktreePath,
    agentSessionId: record.agentSessionId ?? undefined,
  };
}

/** Default task name: the prompt's first line, cut to a readable length. */
export function titleFromPrompt(prompt: string): string {
  const line = prompt.trim().split("\n")[0]?.trim() ?? "";
  return line.length <= 60 ? line : `${line.slice(0, 57).trimEnd()}...`;
}
