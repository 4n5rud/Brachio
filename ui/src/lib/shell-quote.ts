import type { ShellKind } from "./setup-signal";

const SAFE = /^[\w@%+=:,.\/\\-]+$/u;
const DRIVE_PATH = /^[A-Za-z]:\\/u;

/** Writes a path the way the shell expects: Windows shells take `\`, Git Bash takes `C:/x`. */
function shellPath(kind: ShellKind, path: string): string {
  if (kind === "powershell" || kind === "cmd") return path.replaceAll("/", "\\");
  return DRIVE_PATH.test(path) ? path.replaceAll("\\", "/") : path;
}

/** Quotes one path so the shell reads it as a single word. Control characters are dropped. */
export function quoteShellPath(kind: ShellKind, path: string): string {
  const text = shellPath(kind, path).replace(/[\x00-\x1f\x7f-\x9f]/gu, "");
  if (SAFE.test(text) && !(kind === "powershell" && /^[-+=%]/u.test(text))) return text;
  switch (kind) {
    case "powershell":
      // PowerShell also treats curly single quotes as quote characters; doubling each keeps it literal.
      return `'${text.replace(/['\u2018\u2019\u201a\u201b]/gu, "$&$&")}'`;
    case "cmd":
      return `"${text.replaceAll('"', '""')}"`;
    default:
      return `'${text.replaceAll("'", "'\\''")}'`;
  }
}

/** Paths as typed input: quoted, space separated, with a trailing space and no Enter. */
export function quoteShellPaths(kind: ShellKind, paths: readonly string[]): string {
  return paths.map((path) => quoteShellPath(kind, path)).join(" ") + " ";
}
