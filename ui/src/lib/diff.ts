export interface DiffLocation {
  path: string;
  line: number;
  side: "old" | "new";
}

export interface DiffRow {
  text: string;
  location?: DiffLocation;
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
  let oldPath = "";
  let newPath = "";
  let oldLine = 0;
  let newLine = 0;
  let oldRemaining = 0;
  let newRemaining = 0;

  return patch.split("\n").map((text) => {
    if (oldRemaining || newRemaining) {
      const prefix = text[0];
      if (prefix === "-") {
        oldRemaining--;
        return { text, location: { path: oldPath, line: oldLine++, side: "old" } };
      }
      if (prefix === "+") {
        newRemaining--;
        return { text, location: { path: newPath, line: newLine++, side: "new" } };
      }
      if (prefix === " ") {
        oldRemaining--;
        newRemaining--;
        oldLine++;
        return { text, location: { path: newPath, line: newLine++, side: "new" } };
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

function displayPath(path: string, prefix: string): string {
  const name = path.replace(/\t$/, "");
  if (name.startsWith(`"${prefix}`)) return `"${name.slice(prefix.length + 1)}`;
  return name.startsWith(prefix) ? name.slice(prefix.length) : name;
}
