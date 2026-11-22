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
export function rankFiles(
  files: readonly string[],
  query: string,
  limit = MAX_FILE_RESULTS,
  recents: readonly string[] = [],
): string[] {
  const terms = termsOf(query);
  if (query.trim().length < MIN_FILE_QUERY || terms.length === 0) return [];
  const first = terms[0] ?? "";
  const letters = terms.join("");
  const scored: { path: string; score: number; recent: number }[] = [];
  for (const path of files) {
    const lower = path.toLowerCase();
    const name = lower.slice(lower.lastIndexOf("/") + 1);
    const recent = recents.indexOf(path);
    const rank = recent < 0 ? recents.length : recent;
    if (terms.every((term) => lower.includes(term))) {
      scored.push({ path, score: name.startsWith(first) ? 0 : name.includes(first) ? 1 : 2, recent: rank });
    } else if (isSubsequence(letters, name)) {
      scored.push({ path, score: 3, recent: rank });
    }
  }
  scored.sort((a, b) => a.score - b.score || a.recent - b.recent || a.path.length - b.path.length || a.path.localeCompare(b.path));
  return scored.slice(0, limit).map((entry) => entry.path);
}

/** True when the letters appear in order in the text, not necessarily next to each other. */
function isSubsequence(letters: string, text: string): boolean {
  let at = 0;
  for (const letter of text) {
    if (letter === letters[at]) at++;
    if (at === letters.length) return true;
  }
  return letters.length === 0;
}

/** Recently opened files that still exist, in the order they were opened. */
export const existingRecents = (files: readonly string[], recents: readonly string[], limit = 8): string[] => {
  const present = new Set(files);
  return recents.filter((path) => present.has(path)).slice(0, limit);
};

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

export interface TextPart {
  text: string;
  hit: boolean;
}

/** Splits text so the parts that match a query word can be marked. */
export function highlightParts(text: string, query: string): TextPart[] {
  const terms = termsOf(query);
  if (terms.length === 0) return [{ text, hit: false }];
  const lower = text.toLowerCase();
  const hit = new Array<boolean>(text.length).fill(false);
  for (const term of terms) {
    for (let at = lower.indexOf(term); at !== -1; at = lower.indexOf(term, at + term.length)) {
      hit.fill(true, at, at + term.length);
    }
  }
  const parts: TextPart[] = [];
  for (let index = 0; index < text.length; index++) {
    const last = parts[parts.length - 1];
    if (last && last.hit === hit[index]) last.text += text[index];
    else parts.push({ text: text[index] ?? "", hit: hit[index] ?? false });
  }
  return parts;
}
