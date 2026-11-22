export type TerminalLink =
  | { kind: "url"; start: number; end: number; url: string }
  | { kind: "file"; start: number; end: number; path: string; line: number; column?: number };

const URL_PATTERN = /https?:\/\/[^\s<>"'`]+/gu;
// A path with an extension, then :line and an optional :column. A leading slash, dot or word
// character before it means it is part of something longer (a URL, a host:port), so it is skipped.
const FILE_PATTERN = /(?<![\w/\\.@:-])((?:[A-Za-z]:[\\/])?(?:[\w@.+-]+[\\/])*[\w@+-][\w@.+-]*\.[A-Za-z][A-Za-z0-9]{0,7}):(\d{1,7})(?::(\d{1,5}))?/gu;
const TRAILING = /[.,;:!?)\]}>]+$/u;

/** Links in one logical line of terminal text; offsets index into `text`. */
export function findTerminalLinks(text: string): TerminalLink[] {
  const links: TerminalLink[] = [];
  for (const match of text.matchAll(URL_PATTERN)) {
    let url = match[0].replace(TRAILING, "");
    // A closing bracket that has no opening one in the URL belongs to the sentence around it.
    while (url.endsWith(")") && !url.includes("(")) url = url.slice(0, -1);
    try {
      new URL(url);
    } catch {
      continue;
    }
    links.push({ kind: "url", start: match.index, end: match.index + url.length, url });
  }
  for (const match of text.matchAll(FILE_PATTERN)) {
    const start = match.index;
    const end = start + match[0].length;
    if (links.some((link) => start < link.end && end > link.start)) continue;
    const line = Number(match[2]);
    if (line < 1) continue;
    links.push({ kind: "file", start, end, path: match[1] ?? "", line, column: match[3] ? Number(match[3]) : undefined });
  }
  return links.sort((a, b) => a.start - b.start);
}

const normalize = (path: string) => path.replaceAll("\\", "/").replace(/\/+$/u, "");

/**
 * A path from terminal output as a path inside the task folder, or undefined when it points
 * elsewhere. Relative paths are taken as relative to `cwd`, which is where the shell starts.
 */
export function taskRelativePath(path: string, cwd: string): string | undefined {
  const file = normalize(path);
  const root = normalize(cwd);
  const absolute = file.startsWith("/") || /^[A-Za-z]:\//u.test(file);
  let relative: string;
  if (absolute) {
    const windows = /^[A-Za-z]:/u.test(root);
    const same = (a: string, b: string) => (windows ? a.toLowerCase() === b.toLowerCase() : a === b);
    if (file.length <= root.length || !same(file.slice(0, root.length), root) || file[root.length] !== "/") return undefined;
    relative = file.slice(root.length + 1);
  } else {
    relative = file.replace(/^(\.\/)+/u, "");
  }
  const parts = relative.split("/");
  return relative !== "" && parts.every((part) => part !== ".." && part !== "") ? relative : undefined;
}
