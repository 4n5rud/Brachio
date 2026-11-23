import { invoke } from "@tauri-apps/api/core";

export interface Commit {
  hash: string;
  parents: string[];
  author: string;
  committedAtMs: number;
  refs: string[];
  subject: string;
}

export interface CommitPatch {
  patch: string;
  truncated: boolean;
}

export const listCommits = (id: string) => invoke<Commit[]>("task_commit_list", { id });
export const readCommitDiff = (id: string, hash: string) => invoke<CommitPatch>("task_commit_diff", { id, hash });

/** A lane after a row: the commit it is waiting for, and the lane its line comes from. */
export interface GraphLane {
  hash: string;
  from: number;
}

export interface GraphRow {
  column: number;
  /** Lanes that reach this row from above, by the commit each one waits for. */
  before: readonly string[];
  after: readonly GraphLane[];
  /** Lanes already waiting for another parent of this commit, which its line joins. */
  joins: readonly number[];
}

/**
 * Assigns each commit a lane. A lane waits for one commit; the commit takes the first lane that
 * waits for it, its first parent keeps the lane, and further parents join an existing lane or
 * open a new one. Commits must come children first, as `git log --topo-order` lists them.
 */
export function commitGraph(commits: readonly Pick<Commit, "hash" | "parents">[]): GraphRow[] {
  let lanes: string[] = [];
  return commits.map((commit) => {
    const before = lanes;
    let column = before.indexOf(commit.hash);
    if (column < 0) column = before.length;
    const next: GraphLane[] = before.map((hash, index) => ({ hash, from: index }));
    if (column === before.length) next.push({ hash: commit.hash, from: column });
    const joins: number[] = [];
    const [first, ...others] = commit.parents;
    if (first === undefined) next[column] = { hash: "", from: column };
    else next[column] = { hash: first, from: column };
    for (const parent of others) {
      const waiting = next.findIndex((lane) => lane.hash === parent);
      if (waiting < 0) next.push({ hash: parent, from: column });
      else joins.push(waiting);
    }
    // Another lane waiting for the same commit ends here, folding into its column.
    for (let index = 0; index < next.length; index++) {
      if (index !== column && next[index]?.hash === commit.hash) next[index] = { hash: "", from: column };
    }
    const after = trimEnd(next);
    lanes = after.map((lane) => lane.hash);
    return { column, before, after, joins };
  });
}

function trimEnd(lanes: GraphLane[]): GraphLane[] {
  const result = [...lanes];
  while (result.length > 0 && result[result.length - 1]?.hash === "") result.pop();
  return result;
}
