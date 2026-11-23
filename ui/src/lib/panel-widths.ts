const SIDEBAR_KEY = "brachio.sidebarWidth";
const INSPECTOR_KEY = "brachio.inspectorWidth";

export type ResizablePanel = "sidebar" | "inspector";

const MIN_WIDTH: Record<ResizablePanel, number> = { sidebar: 200, inspector: 320 };
const MAX_WIDTH: Record<ResizablePanel, number> = { sidebar: 480, inspector: 720 };
/** The task view keeps at least this much room, whatever the panels ask for. */
const MIN_CENTER = 352;

/**
 * Fits a stored or dragged width to the window: within the panel's own range, and leaving the
 * task view its room next to the other panel (`otherWidth`, 0 when that panel is closed).
 */
export function clampPanelWidth(panel: ResizablePanel, width: number, windowWidth: number, otherWidth: number): number {
  const room = windowWidth - otherWidth - MIN_CENTER;
  const max = Math.max(MIN_WIDTH[panel], Math.min(MAX_WIDTH[panel], room));
  return Math.round(Math.min(max, Math.max(MIN_WIDTH[panel], width)));
}

function load(key: string): number | undefined {
  try {
    const value = Number(localStorage.getItem(key));
    return Number.isFinite(value) && value > 0 ? value : undefined;
  } catch {
    return undefined;
  }
}

function save(key: string, width: number): void {
  try {
    localStorage.setItem(key, String(width));
  } catch {
    // The width still applies for this run.
  }
}

export const loadSidebarWidth = () => load(SIDEBAR_KEY);
export const loadInspectorWidth = () => load(INSPECTOR_KEY);
export const saveSidebarWidth = (width: number) => save(SIDEBAR_KEY, width);
export const saveInspectorWidth = (width: number) => save(INSPECTOR_KEY, width);
