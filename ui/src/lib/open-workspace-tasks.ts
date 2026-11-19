const OPEN_TASKS_KEY = "brachio.openWorkspaceTasks";

export function loadOpenWorkspaceTasks(active: ReadonlySet<string>): string[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(OPEN_TASKS_KEY) ?? "[]");
    if (!Array.isArray(stored)) return [];
    return [...new Set(stored.filter((id): id is string => typeof id === "string" && active.has(id)))];
  } catch {
    return [];
  }
}

export function saveOpenWorkspaceTasks(ids: readonly string[]) {
  try {
    localStorage.setItem(OPEN_TASKS_KEY, JSON.stringify(ids));
  } catch {
    // The list remains available for this run.
  }
}
