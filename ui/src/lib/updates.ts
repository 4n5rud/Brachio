import { invoke } from "@tauri-apps/api/core";

const ENDPOINT = "https://api.github.com/repos/4n5rud/Brachio/releases/latest";
const VERSION = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9a-zA-Z.-]+)?(?:\+[0-9a-zA-Z.-]+)?$/;

/** A published stable version beats the same version's release candidate. */
export function newerStableVersion(installed: string, tag: string): string | null {
  const current = VERSION.exec(installed);
  const release = VERSION.exec(tag);
  if (!current || !release || release[4]) return null;
  const version = release.slice(1, 4).join(".");
  for (const index of [1, 2, 3]) {
    const next = BigInt(release[index] ?? "");
    const previous = BigInt(current[index] ?? "");
    if (next !== previous) return next > previous ? version : null;
  }
  return current[4] ? version : null;
}

export async function checkForRelease(): Promise<string | null> {
  const installed = await invoke<string>("app_version");
  const response = await fetch(ENDPOINT, {
    headers: { Accept: "application/vnd.github+json" },
    signal: AbortSignal.timeout(10_000),
  });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`Release check failed (${response.status}).`);
  const release = await response.json();
  return typeof release?.tag_name === "string" && release.draft === false && release.prerelease === false
    ? newerStableVersion(installed, release.tag_name)
    : null;
}

export const openRelease = () => invoke<void>("open_update_release");
