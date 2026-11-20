export interface DiffLocation {
  path: string;
  line: number;
  side: "old" | "new";
}

export interface DiffRow {
  text: string;
  location?: DiffLocation;
}

export interface DiffTotals {
  additions: number;
  deletions: number;
}

/** Counts text changes in complete patch hunks, excluding headers and context. */
export function diffTotals(patch: string): DiffTotals {
  let additions = 0;
  let deletions = 0;
  for (const { text, location } of diffRows(patch)) {
    if (location?.side === "old") deletions++;
    if (location?.side === "new" && text.startsWith("+")) additions++;
  }
  return { additions, deletions };
}

export type SplitDiffRow = { shared: DiffRow } | { old?: DiffRow; next?: DiffRow };

export function splitDiffRows(rows: readonly DiffRow[]): SplitDiffRow[] {
  const result: SplitDiffRow[] = [];
  let removed: DiffRow[] = [];
  let added: DiffRow[] = [];
  const flush = () => {
    for (let index = 0; index < Math.max(removed.length, added.length); index++) {
      result.push({ old: removed[index], next: added[index] });
    }
    removed = [];
    added = [];
  };

  for (const row of rows) {
    if (row.location?.side === "old") {
      if (added.length) flush();
      removed.push(row);
    } else if (row.location && row.text.startsWith("+")) {
      added.push(row);
    } else {
      flush();
      result.push({ shared: row });
    }
  }
  flush();
  return result;
}

export function diffRows(patch: string): DiffRow[] {
  let oldPath: string | null = "";
  let newPath: string | null = "";
  let oldLine = 0;
  let newLine = 0;
  let oldRemaining = 0;
  let newRemaining = 0;

  return patch.split("\n").map((text) => {
    if (oldRemaining || newRemaining) {
      const prefix = text[0];
      if (prefix === "-") {
        oldRemaining--;
        const line = oldLine++;
        return oldPath === null ? { text } : { text, location: { path: oldPath, line, side: "old" } };
      }
      if (prefix === "+") {
        newRemaining--;
        const line = newLine++;
        return newPath === null ? { text } : { text, location: { path: newPath, line, side: "new" } };
      }
      if (prefix === " ") {
        oldRemaining--;
        newRemaining--;
        oldLine++;
        const line = newLine++;
        return newPath === null ? { text } : { text, location: { path: newPath, line, side: "new" } };
      }
    }
    if (text.startsWith("diff --git ")) {
      oldPath = "";
      newPath = "";
    } else if (text.startsWith("--- ")) {
      oldPath = displayPath(text.slice(4), "a/");
    } else if (text.startsWith("+++ ")) {
      newPath = displayPath(text.slice(4), "b/");
    } else {
      const hunk = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/.exec(text);
      if (hunk) {
        oldLine = Number(hunk[1]);
        oldRemaining = Number(hunk[2] ?? 1);
        newLine = Number(hunk[3]);
        newRemaining = Number(hunk[4] ?? 1);
      }
    }
    return { text };
  });
}

const SIMPLE_ESCAPES: Record<string, number> = {
  a: 7, b: 8, f: 12, n: 10, r: 13, t: 9, v: 11, '"': 34, "\\": 92,
};

// Decodes Git's C-style quoted path; null when malformed so callers drop the location.
function unquoteGitPath(quoted: string): string | null {
  const bytes: number[] = [];
  const encoder = new TextEncoder();
  let i = 1;
  while (i < quoted.length) {
    const ch = quoted[i] ?? "";
    if (ch === '"') {
      if (quoted.slice(i + 1).replace(/\t$/, "") !== "") return null;
      try {
        return new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(bytes));
      } catch {
        return null;
      }
    }
    if (ch !== "\\") {
      bytes.push(...encoder.encode(ch));
      i += ch.length;
      continue;
    }
    const octal = /^[0-3][0-7]{2}/.exec(quoted.slice(i + 1, i + 4));
    if (octal) {
      bytes.push(parseInt(octal[0], 8));
      i += 4;
    } else if (SIMPLE_ESCAPES[quoted[i + 1] ?? ""] !== undefined) {
      bytes.push(SIMPLE_ESCAPES[quoted[i + 1] ?? ""]!);
      i += 2;
    } else {
      return null;
    }
  }
  return null;
}

function displayPath(path: string, prefix: string): string | null {
  const name = path.startsWith('"') ? unquoteGitPath(path) : path.replace(/\t$/, "");
  if (name === null) return null;
  return name.startsWith(prefix) ? name.slice(prefix.length) : name;
}

export type DiffFileStatus = "A" | "M" | "D" | "R";

export interface DiffFile {
  path: string;
  status: DiffFileStatus;
  additions: number;
  deletions: number;
}

/** One entry per `diff --git` section: the path after the change, how it changed and its line totals. */
export function diffFiles(patch: string): DiffFile[] {
  const files: DiffFile[] = [];
  let current: DiffFile | undefined;
  let hunkRemaining = false;
  const lines = patch.split("\n");
  diffRows(patch).forEach((row, index) => {
    const text = lines[index] ?? "";
    if (row.location) {
      if (!current) return;
      if (row.location.side === "old") current.deletions++;
      else if (text.startsWith("+")) current.additions++;
      return;
    }
    if (text.startsWith("diff --git ")) {
      const names = /^diff --git (?:"?a\/.*?"? )?(".*"|b\/.*)$/.exec(text)?.[1] ?? "";
      current = { path: displayPath(names, "b/") ?? "", status: "M", additions: 0, deletions: 0 };
      files.push(current);
      hunkRemaining = false;
    } else if (!current || hunkRemaining) {
      return;
    } else if (text.startsWith("new file mode")) current.status = "A";
    else if (text.startsWith("deleted file mode")) current.status = "D";
    else if (text.startsWith("rename to ")) {
      current.status = "R";
      current.path = text.slice("rename to ".length);
    } else if (text.startsWith("@@")) hunkRemaining = true;
  });
  return files;
}
