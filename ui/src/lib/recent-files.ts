const LIMIT = 20;
const key = (taskId: string) => `brachio.recentFiles.${taskId}`;

/** Files opened from quick open for this task, newest first. */
export function recentFiles(taskId: string): string[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(key(taskId)) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string") : [];
  } catch {
    return [];
  }
}

export function noteRecentFile(taskId: string, path: string): void {
  try {
    const next = [path, ...recentFiles(taskId).filter((item) => item !== path)].slice(0, LIMIT);
    localStorage.setItem(key(taskId), JSON.stringify(next));
  } catch {
    // Recents are a convenience; without storage the list is simply empty.
  }
}
