import { invoke } from "@tauri-apps/api/core";
import type { AgentKind as CoreAgent } from "./terminal";
import type { CreatedTask, RepoRecord } from "./workspace";

/** A folder under Brachio's scratch folder: a scratch project not turned into a project yet, or a folder left behind. */
export interface ScratchFolder {
  /** Empty for a folder left behind without a project. */
  repoId: string | null;
  name: string;
  path: string;
  /** Titles of the tasks that ran in it. */
  tasks: string[];
  /** The folder is gone and only the project remains. */
  missing: boolean;
}

/** Starts an agent in a new temporary folder without git. */
export const createScratchTask = (request: { title: string; prompt: string; agent: CoreAgent }) =>
  invoke<CreatedTask>("scratch_create", { request });

export const listScratchFolders = () => invoke<ScratchFolder[]>("scratch_list");

/** Moves the folder to `parent/name`, runs `git init` there and registers it as a project. */
export const convertScratch = (repoId: string, parent: string, name: string) =>
  invoke<RepoRecord>("scratch_convert", { request: { repoId, parent, name } });

/** Deletes the folder with its project and tasks. */
export const discardScratch = (path: string) => invoke<void>("scratch_discard", { path });

/** Sent when a scratch folder became a project or went away, so lists reload. */
export const PROJECTS_CHANGED = "brachio:projects-changed";
