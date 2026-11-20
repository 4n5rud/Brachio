import type { TaskRow } from "./tasks";

/** What the command entries of quick open can do. */
export type CommandId = "newTask" | "settings" | "archived";

export type QuickEntry =
  | { kind: "task"; task: TaskRow }
  | { kind: "command"; id: CommandId }
  | { kind: "file"; path: string };

/** A file search starts after this many characters, so the whole list is not shown. */
export const MIN_FILE_QUERY = 2;
export const MAX_FILE_RESULTS = 30;

const termsOf = (query: string): string[] => query.trim().toLowerCase().split(/\s+/).filter(Boolean);

/** True when every word of the query appears in the text, in any order. */
export const matchesAll = (text: string, query: string): boolean => {
  const lower = text.toLowerCase();
  return termsOf(query).every((term) => lower.includes(term));
};

/**
 * Files whose path contains every word of the query. A file whose name starts with the first word
 * comes before one whose name only contains it, and both come before a match in a folder name.
 * Shorter paths come first within a group.
 */
export function rankFiles(files: readonly string[], query: string, limit = MAX_FILE_RESULTS): string[] {
  const terms = termsOf(query);
  if (query.trim().length < MIN_FILE_QUERY || terms.length === 0) return [];
  const first = terms[0] ?? "";
  const scored: { path: string; score: number }[] = [];
  for (const path of files) {
    const lower = path.toLowerCase();
    if (!terms.every((term) => lower.includes(term))) continue;
    const name = lower.slice(lower.lastIndexOf("/") + 1);
    scored.push({ path, score: name.startsWith(first) ? 0 : name.includes(first) ? 1 : 2 });
  }
  scored.sort((a, b) => a.score - b.score || a.path.length - b.path.length || a.path.localeCompare(b.path));
  return scored.slice(0, limit).map((entry) => entry.path);
}

/** Loads a file list and applies it unless cancelled first, so a slower earlier request cannot overwrite a newer one. */
export function loadFilesUnlessCancelled(
  load: () => Promise<readonly string[]>,
  apply: (files: readonly string[]) => void,
): () => void {
  let cancelled = false;
  load().then(
    (files) => { if (!cancelled) apply(files); },
    () => { if (!cancelled) apply([]); },
  );
  return () => { cancelled = true; };
}
