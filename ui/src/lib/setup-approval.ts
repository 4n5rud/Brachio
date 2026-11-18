const STORAGE_KEY = "brachio.approvedSetups";

/** Setup command lists the user has agreed to run, each as the repository id and its commands. */
export function loadApprovedSetups(): string[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(stored) ? stored.filter((entry): entry is string => typeof entry === "string") : [];
  } catch {
    return [];
  }
}

export function saveApprovedSetups(entries: readonly string[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // Without storage the question is simply asked again.
  }
}
