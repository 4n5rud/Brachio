/** Which terminals a task shows: its agent, a shell, or both side by side. */
export type TerminalLayout = "agent" | "shell" | "split";

const STORAGE_KEY = "brachio.terminalLayouts";
const LAYOUTS: readonly TerminalLayout[] = ["agent", "shell", "split"];

const isLayout = (value: unknown): value is TerminalLayout =>
  typeof value === "string" && (LAYOUTS as readonly string[]).includes(value);

/** The layouts remembered from the last run, or none when storage is blocked or damaged. */
export function loadLayouts(): Map<string, TerminalLayout> {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (stored === null || typeof stored !== "object") return new Map();
    return new Map(Object.entries(stored).filter((entry): entry is [string, TerminalLayout] => isLayout(entry[1])));
  } catch {
    return new Map();
  }
}

export function saveLayouts(layouts: ReadonlyMap<string, TerminalLayout>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(Object.fromEntries(layouts)));
  } catch {
    // Without storage the layout is simply not remembered.
  }
}

/** Stable name of a task's terminal, so the runtime can find it again after a restart. */
export const terminalKey = (taskId: string, kind: "agent" | "shell"): string => `${taskId}:${kind}`;
