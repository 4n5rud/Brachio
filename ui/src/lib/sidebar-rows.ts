import { createSignal } from "solid-js";

/** Which parts of a task row the sidebar shows. The title and status glyph always show. */
export interface SidebarRowParts {
  agent: boolean;
  branch: boolean;
  changes: boolean;
  statusText: boolean;
}

const STORAGE_KEY = "brachio.sidebarRows";

/** Everything stays on unless it was switched off. */
export function normalizeRowParts(raw: unknown): SidebarRowParts {
  const fields = raw !== null && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  return {
    agent: fields.agent !== false,
    branch: fields.branch !== false,
    changes: fields.changes !== false,
    statusText: fields.statusText !== false,
  };
}

function load(): SidebarRowParts {
  try {
    return normalizeRowParts(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}"));
  } catch {
    return normalizeRowParts({});
  }
}

const [parts, setParts] = createSignal(load());

export const sidebarRowParts = parts;

export function updateSidebarRowParts(change: Partial<SidebarRowParts>): void {
  const next = { ...parts(), ...change };
  setParts(next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // The choice lasts until the app closes.
  }
}
