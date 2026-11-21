/** Why a repository URL cannot be cloned, or null when it can. Mirrors the checks the core makes. */
export type CloneUrlProblem = "empty" | "scheme" | "format";

export function cloneUrlProblem(raw: string): CloneUrlProblem | null {
  const url = raw.trim();
  if (url === "") return "empty";
  if (/[\s\u0000-\u001f]/.test(url) || url.length > 2048 || url.startsWith("-")) return "format";
  const lower = url.toLowerCase();
  for (const scheme of ["https://", "ssh://"]) {
    if (lower.startsWith(scheme)) {
      const rest = lower.slice(scheme.length);
      const host = (rest.split(/[/:]/)[0] ?? "").split("@").pop() ?? "";
      return host === "" || !rest.includes("/") ? "format" : null;
    }
  }
  if (lower.includes("://") || lower.includes("::")) return "scheme";
  const at = url.indexOf(":");
  const host = at > 0 ? url.slice(0, at) : "";
  const path = at > 0 ? url.slice(at + 1) : "";
  const scpLike = host.includes("@") && !host.includes("/") && !host.startsWith("@") && !host.endsWith("@")
    && path !== "" && !path.startsWith("/");
  return scpLike ? null : "scheme";
}

/** The path parts of a repository URL, for naming its folder and its `owner/repo` label. */
function urlParts(url: string): string[] {
  return url.trim().replace(/\/+$/, "").replace(/\.git$/, "").split(/[/:]/).filter((part) => part !== "");
}

/** The folder a URL names: its last path part without `.git`. */
export function folderNameForUrl(url: string): string {
  const name = urlParts(url).pop() ?? "";
  return name === "." || name === ".." ? "" : name;
}

/** `owner/repo` from a remote URL, or null when it has no two-part path. */
export function repoSlug(url: string | undefined): string | null {
  if (!url) return null;
  const parts = urlParts(url);
  if (parts.length < 2) return null;
  return parts.slice(-2).join("/");
}
