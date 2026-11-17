import { invoke } from "@tauri-apps/api/core";

export interface CreatedWorktree {
  branch: string;
  path: string;
}

export const createWorktree = (repo: string, title: string, base?: string) =>
  invoke<CreatedWorktree>("worktree_create", { request: { repo, title, base: base ?? null } });
