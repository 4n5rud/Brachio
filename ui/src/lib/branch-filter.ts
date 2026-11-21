import { invoke } from "@tauri-apps/api/core";

/** A branch a task can start from, as the backend lists it. */
export interface BranchChoice {
  name: string;
  /** What to hand to git as the base. */
  value: string;
  local: boolean;
  remote: string | null;
}

export const listBranches = (id: string) => invoke<BranchChoice[]>("repo_branches", { id });

export const MAX_SHOWN_BRANCHES = 8;

/** Branches whose name contains the typed text, names that start with it first. Order within each
 * group stays as listed, newest first. */
export function filterBranches(branches: readonly BranchChoice[], query: string): BranchChoice[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return branches.slice(0, MAX_SHOWN_BRANCHES);
  const starts: BranchChoice[] = [];
  const inside: BranchChoice[] = [];
  for (const branch of branches) {
    const name = branch.name.toLowerCase();
    if (name.startsWith(needle)) starts.push(branch);
    else if (name.includes(needle)) inside.push(branch);
  }
  return [...starts, ...inside].slice(0, MAX_SHOWN_BRANCHES);
}

/** Where a branch exists: "local", "origin" or "local and origin". */
export function branchWhere(branch: BranchChoice, localLabel: string, andLabel: string): string {
  if (branch.local && branch.remote) return `${localLabel} ${andLabel} ${branch.remote}`;
  return branch.remote ?? localLabel;
}
