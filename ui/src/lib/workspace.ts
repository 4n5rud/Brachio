import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { AgentKind as CoreAgent, CodexConversation } from "./terminal";
import { agentOfCore } from "./agent-catalog";
import type { AgentKind, TaskRow } from "./tasks";

/** A repository as the core stores it. */
export interface RepoRecord {
  id: string;
  name: string;
  path: string;
  defaultBranch: string;
  addedAtMs: number;
  /** False for a plain folder: its tasks run in the folder, with no branch or worktree. */
  isGit: boolean;
}

/** A task as the core stores it. */
export interface TaskRecord {
  id: string;
  repoId: string;
  parentTaskId?: string | null;
  title: string;
  prompt?: string;
  branch: string;
  worktreePath: string;
  agentKind: CoreAgent | null;
  createdAtMs: number;
  archivedAtMs: number | null;
  /** Conversation id the agent resumes, when Brachio picked one. */
  agentSessionId: string | null;
  /** Shared by the sibling tasks that run one prompt with different agents. */
  compareGroup?: string | null;
}

export interface CreateTaskRequest {
  repoId: string;
  parentTaskId?: string | null;
  title: string;
  prompt?: string;
  agent: CoreAgent;
  /** `null` branches from the repository's default branch. */
  base: string | null;
  /** A Codex conversation recorded for the project folder, continued by the new task. */
  codexConversationId?: string | null;
  /** Marks the task as one of several run on one prompt to compare agents. */
  compareGroup?: string | null;
  /** A profile from the repository's `brachio.toml`, applied whenever the agent starts. */
  profile?: string | null;
}

export const listRepos = () => invoke<RepoRecord[]>("repo_list");
/** Saves the order the projects were dragged into. */
export const reorderRepos = (ids: readonly string[]) => invoke<void>("repo_reorder", { ids });
/** Saves the order one project's tasks were dragged into. */
export const reorderTasks = (ids: readonly string[]) => invoke<void>("task_reorder", { ids });
/** With `gitOnly`, a folder that is not a Git repository is refused instead of added as a folder workspace. */
export const addRepo = (path: string, gitOnly = false) => invoke<RepoRecord>("repo_add", { path, gitOnly });
/** Clones `url` into a new folder inside `parent` and adds it. Rejects with "cancelled" when stopped. */
export const cloneProject = (jobId: string, url: string, parent: string, name?: string) =>
  invoke<RepoRecord>("project_clone", { jobId, url, parent, name: name || null });
export const cancelClone = (jobId: string) => invoke<void>("project_clone_cancel", { jobId });
/** Calls `onPercent` with the overall progress, 0 to 100, of the clone with this id. */
export const watchCloneProgress = (jobId: string, onPercent: (percent: number) => void) =>
  listen<{ jobId: string; percent: number }>("project-clone-progress", (event) => {
    if (event.payload.jobId === jobId) onPercent(event.payload.percent);
  });
/** Creates a folder inside `parent`, runs `git init` in it and adds it. */
export const initProject = (parent: string, name: string) => invoke<RepoRecord>("project_init", { parent, name });
/** `origin` URL of each project that has one, by project id. */
export const listRemoteUrls = () => invoke<Record<string, string>>("repo_remote_urls");

/** A git repository found inside a folder project. */
export interface FolderRepo {
  path: string;
  name: string;
  /** Already a project of its own. */
  added: boolean;
}
export const listFolderRepos = (id: string) => invoke<FolderRepo[]>("folder_source_repos", { id });
/** A Git worktree of a project that no task uses yet. */
export interface ImportableWorktree {
  path: string;
  branch: string;
}
export const listImportableWorktrees = (repoId: string) =>
  invoke<ImportableWorktree[]>("worktree_import_candidates", { repoId });
export const importWorktrees = (repoId: string, paths: string[]) =>
  invoke<TaskRecord[]>("worktree_import", { repoId, paths });
export const listTasks = () => invoke<TaskRecord[]>("task_list");
/** A task that was just created, with the setup commands its repository asks to run. */
export type CreatedTask = TaskRecord & { setupCommands: string[]; setupCopyError?: string | null };
export const createTask = (request: CreateTaskRequest) => invoke<CreatedTask>("task_create", { request });
export const repoCodexConversations = (id: string) =>
  invoke<CodexConversation[]>("repo_codex_conversations", { id });
export const retrySetupCopy = (id: string) => invoke<void>("task_retry_setup_copy", { id });
export const archiveTask = (id: string) => invoke<void>("task_archive", { id });
export const listArchivedTasks = () => invoke<TaskRecord[]>("task_list_archived");
export const restoreTask = (id: string) => invoke<TaskRecord>("task_restore", { id });

/** What deleting an archived task would destroy. */
export interface TaskLoss {
  uncommittedFiles: number;
  unpushedCommits: number;
  /** Files git ignores, never committed, such as `.env.local`. */
  ignoredEntries: number;
  ignoredExamples: string[];
  /** Identifies the state shown; `deleteTask` must send it back. */
  fingerprint: string;
  runningTerminals: number;
  /** The terminal check failed, so `runningTerminals` means nothing. */
  terminalsUnknown: boolean;
}
export const previewTaskDeletion = (id: string) => invoke<TaskLoss>("task_delete_preview", { id });
export const listTaskFiles = (id: string) => invoke<string[]>("task_files", { id });
export const previewTaskFile = (id: string, path: string) =>
  invoke<{ text: string | null; truncated: boolean; tooLarge: boolean }>("task_file_preview", { id, path });
export const previewTaskMedia = (id: string, path: string) =>
  invoke<ArrayBuffer | number[]>("task_file_media", { id, path });
export const revealPath = (path: string) => invoke<void>("path_reveal", { path });
export const revealTask = (id: string) => invoke<void>("task_reveal", { id });
export const openTaskFolder = (id: string) => invoke<void>("task_open_folder", { id });
export const openLogFolder = () => invoke<void>("open_log_folder");
export const deleteTask = (id: string, expectedFingerprint: string) =>
  invoke<void>("task_delete", { id, expectedFingerprint });

interface TaskChanges {
  patch: string;
  untracked: string[];
  staged: string[];
  conflicts: string[];
  /** A rebase or merge that Git stopped in the worktree. */
  operation?: "rebase" | "merge" | null;
  truncated: boolean;
}

export const readTaskChanges = (id: string) => invoke<TaskChanges>("task_changes", { id });

export interface TaskMergePreview {
  baseBranch: string;
  conflicts: string[];
  /** False when no remote copy of the base branch exists and the local branch was used. */
  remoteTracking: boolean;
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

const UI_AGENT = (core: CoreAgent): AgentKind => agentOfCore(core);

/**
 * Shapes a stored task for the list. Live status and diff counts are not
 * loaded yet, so the row starts without line totals.
 */
export function toTaskRow(record: TaskRecord): TaskRow {
  return {
    id: record.id,
    repoId: record.repoId,
    title: record.title,
    prompt: record.prompt || undefined,
    agent: UI_AGENT(record.agentKind ?? "claude_code"),
    branch: record.branch,
    status: { state: "unknown" },
    updatedAt: record.createdAtMs,
    archivedAt: record.archivedAtMs ?? undefined,
    worktreePath: record.worktreePath,
    agentSessionId: record.agentSessionId ?? undefined,
    compareGroup: record.compareGroup ?? undefined,
    parentTaskId: record.parentTaskId ?? undefined,
  };
}

/** Default task name: the prompt's first line, cut to a readable length. */
export function titleFromPrompt(prompt: string): string {
  const line = prompt.trim().split("\n")[0]?.trim() ?? "";
  return line.length <= 60 ? line : `${line.slice(0, 57).trimEnd()}...`;
}

export const resolveDroppedTaskPaths = (id: string, paths: string[]) =>
  invoke<string[]>("task_drop_paths", { id, paths });

export const moveTaskPath = (id: string, from: string, toDir: string) =>
  invoke<string>("task_move_path", { id, from, toDir });
/** Names in the folder that copying the dropped paths there would replace. */
export const taskCopyConflicts = (id: string, sources: string[], toDir: string) =>
  invoke<string[]>("task_copy_conflicts", { id, sources, toDir });
export const copyIntoTask = (id: string, sources: string[], toDir: string, overwrite: boolean) =>
  invoke<string[]>("task_copy_in", { id, sources, toDir, overwrite });

export const renameTaskPath = (id: string, from: string, newName: string) =>
  invoke<string>("task_file_rename", { id, from, newName });
export const createTaskEntry = (id: string, dir: string, name: string, folder: boolean) =>
  invoke<string>("task_file_create", { id, dir, name, folder });
/** Moves to the system trash; nothing is deleted for good. Resolves to whether `restoreTaskPath` can bring it back. */
export const trashTaskPath = (id: string, path: string) => invoke<boolean>("task_file_trash", { id, path });
export const restoreTaskPath = (id: string, path: string) => invoke<void>("task_file_restore", { id, path });
export const discardTaskFile = (id: string, path: string) => invoke<void>("task_discard_file", { id, path });
export const discardTaskHunk = (id: string, patch: string) => invoke<void>("task_discard_hunk", { id, patch });
