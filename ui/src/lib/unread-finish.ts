import type { AgentState } from "./status";

const UNREAD_KEY = "brachio.unreadFinish";

/** An agent run ended in a state that is ready for a person to look at. */
export function isFinish(previous: AgentState, next: AgentState): boolean {
  if (previous.state !== "working") return false;
  return next.state === "idle" || next.state === "exited" && next.code === 0;
}

/** A finish counts as unread only when the person was not looking at that task. */
export function finishedUnseen(previous: AgentState, next: AgentState, watching: boolean): boolean {
  return !watching && isFinish(previous, next);
}

export function loadUnreadFinish(active: ReadonlySet<string>): Set<string> {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(UNREAD_KEY) ?? "[]");
    if (!Array.isArray(stored)) return new Set();
    return new Set(stored.filter((id): id is string => typeof id === "string" && active.has(id)));
  } catch {
    return new Set();
  }
}

export function saveUnreadFinish(ids: ReadonlySet<string>): void {
  try {
    localStorage.setItem(UNREAD_KEY, JSON.stringify([...ids]));
  } catch {
    // The marks still apply for this run.
  }
}
