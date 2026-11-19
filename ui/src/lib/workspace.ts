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
  parentTaskId?: string | null;
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
  parentTaskId?: string | null;
  title: string;
  agent: CoreAgent;
  /** `null` branches from the repository's default branch. */
  base: string | null;
}

export const listRepos = () => invoke<RepoRecord[]>("repo_list");
export const addRepo = (path: string) => invoke<RepoRecord>("repo_add", { path });
export const listTasks = () => invoke<TaskRecord[]>("task_list");
/** A task that was just created, with the setup commands its repository asks to run. */
export type CreatedTask = TaskRecord & { setupCommands: string[] };
export const createTask = (request: CreateTaskRequest) => invoke<CreatedTask>("task_create", { request });
export const archiveTask = (id: string) => invoke<void>("task_archive", { id });
export const listArchivedTasks = () => invoke<TaskRecord[]>("task_list_archived");
export const restoreTask = (id: string) => invoke<TaskRecord>("task_restore", { id });

/** What deleting an archived task would destroy. */
export interface TaskLoss {
  uncommittedFiles: number;
  unpushedCommits: number;
}
export const previewTaskDeletion = (id: string) => invoke<TaskLoss>("task_delete_preview", { id });
export const listTaskFiles = (id: string) => invoke<string[]>("task_files", { id });
export const previewTaskFile = (id: string, path: string) =>
  invoke<{ text: string | null; truncated: boolean }>("task_file_preview", { id, path });
export const previewTaskMedia = (id: string, path: string) =>
  invoke<ArrayBuffer | number[]>("task_file_media", { id, path });
export const deleteTask = (id: string) => invoke<void>("task_delete", { id });

interface TaskChanges {
  patch: string;
  untracked: string[];
  staged: string[];
  conflicts: string[];
  truncated: boolean;
}

export const readTaskChanges = (id: string) => invoke<TaskChanges>("task_changes", { id });

export interface TaskMergePreview {
  baseBranch: string;
  conflicts: string[];
}

export const previewTaskMerge = (id: string) => invoke<TaskMergePreview>("task_merge_preview", { id });
export type TaskGitAction =
  | { action: "stageTracked" | "push" }
  | { action: "stageFile" | "unstageFile"; path: string }
  | { action: "commit"; message: string }
  | { action: "createPr"; title: string; body: string };
/** Resolves to the pull request's address for `createPr`, and to nothing for the other actions. */
export const runTaskGitAction = (id: string, request: TaskGitAction) =>
  invoke<string | null>("task_git_action", { id, request });

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

export const resolveDroppedTaskPaths = (id: string, paths: string[]) =>
  invoke<string[]>("task_drop_paths", { id, paths });
