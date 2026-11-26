/** The syntax modes the file tab's code editor has. Anything else opens as plain text. */
export type EditorLanguage =
  | "markdown"
  | "json"
  | "yaml"
  | "toml"
  | "javascript"
  | "jsx"
  | "typescript"
  | "tsx"
  | "rust"
  | "python"
  | "shell";

const BY_EXTENSION: Readonly<Record<string, EditorLanguage>> = {
  md: "markdown",
  markdown: "markdown",
  mdx: "markdown",
  json: "json",
  jsonc: "json",
  json5: "json",
  yml: "yaml",
  yaml: "yaml",
  toml: "toml",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "tsx",
  rs: "rust",
  py: "python",
  pyi: "python",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
};

/** Files known by their whole name rather than an extension. */
const BY_NAME: Readonly<Record<string, EditorLanguage>> = {
  "cargo.lock": "toml",
  ".bashrc": "shell",
  ".zshrc": "shell",
  ".profile": "shell",
  ".bash_profile": "shell",
  ".envrc": "shell",
};

/** The mode for a path inside the task, or undefined for plain text. */
export function editorLanguageOf(path: string): EditorLanguage | undefined {
  const name = path.replaceAll("\\", "/").split("/").pop()?.toLowerCase() ?? "";
  const named = BY_NAME[name];
  if (named) return named;
  const dot = name.lastIndexOf(".");
  return dot <= 0 ? undefined : BY_EXTENSION[name.slice(dot + 1)];
}

/** How new indentation is typed: what the file already uses, so an edit matches it. */
export interface IndentStyle {
  /** Inserted for one level: a tab, or a number of spaces. */
  unit: string;
  /** How wide a tab is drawn. */
  tabSize: number;
}

const DEFAULT_INDENT: IndentStyle = { unit: "  ", tabSize: 4 };

/**
 * Guesses the indentation a file uses from its lines: tabs when most indented lines start with a
 * tab, otherwise the smallest step between space indents that most lines agree on (2, 4 or 8).
 * Only what the person types follows it; the file's existing text is never changed.
 */
export function detectIndent(text: string): IndentStyle {
  let tabs = 0;
  let spaced = 0;
  const steps = new Map<number, number>();
  let previous = 0;
  let lines = 0;
  for (const line of text.split("\n")) {
    if (++lines > 2000) break;
    if (line.trim() === "") continue;
    if (line.startsWith("\t")) {
      tabs += 1;
      continue;
    }
    const width = line.length - line.trimStart().length;
    if (width > 0) spaced += 1;
    const step = Math.abs(width - previous);
    if (step > 0 && step <= 8) steps.set(step, (steps.get(step) ?? 0) + 1);
    previous = width;
  }
  if (tabs > spaced) return { unit: "\t", tabSize: 4 };
  let best = 0;
  let bestCount = 0;
  for (const step of [2, 4, 8]) {
    const count = steps.get(step) ?? 0;
    if (count > bestCount) {
      best = step;
      bestCount = count;
    }
  }
  return best === 0 ? DEFAULT_INDENT : { unit: " ".repeat(best), tabSize: best };
}

/**
 * The one change that turns `before` into `after`: the differing middle between their common
 * start and end. A reload from disk applies only this, so the cursor and folds outside it stay.
 */
export function middleChange(before: string, after: string): { from: number; to: number; insert: string } | undefined {
  if (before === after) return undefined;
  const limit = Math.min(before.length, after.length);
  let start = 0;
  while (start < limit && before.charCodeAt(start) === after.charCodeAt(start)) start += 1;
  let end = 0;
  while (end < limit - start && before.charCodeAt(before.length - 1 - end) === after.charCodeAt(after.length - 1 - end)) end += 1;
  // Do not split a surrogate pair at either edge.
  if (start > 0 && isHighSurrogate(before.charCodeAt(start - 1))) start -= 1;
  if (end > 0 && isLowSurrogate(before.charCodeAt(before.length - end))) end -= 1;
  return { from: start, to: before.length - end, insert: after.slice(start, after.length - end) };
}

const isHighSurrogate = (code: number) => code >= 0xd800 && code <= 0xdbff;
const isLowSurrogate = (code: number) => code >= 0xdc00 && code <= 0xdfff;
