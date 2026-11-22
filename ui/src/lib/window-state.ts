import { availableMonitors, getCurrentWindow, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";

const STORAGE_KEY = "brachio.windowState";
/** The window minimum in logical pixels, as set in tauri.conf.json. */
const MIN_WIDTH = 1000;
const MIN_HEIGHT = 600;

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WindowState extends Rect {
  maximized: boolean;
}

const overlap = (a: Rect, b: Rect): number => {
  const width = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
  const height = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
  return width > 0 && height > 0 ? width * height : 0;
};

/**
 * Where the window may open: on the monitor it overlapped most (the first one when its monitor
 * is gone), no larger than that monitor, no smaller than the minimum, fully on screen.
 */
export function clampToMonitors(saved: Rect, monitors: readonly Rect[], minWidth = MIN_WIDTH, minHeight = MIN_HEIGHT): Rect | undefined {
  const first = monitors[0];
  if (!first) return undefined;
  const monitor = monitors.reduce((best, candidate) => (overlap(saved, candidate) > overlap(saved, best) ? candidate : best), first);
  const width = Math.min(monitor.width, Math.max(minWidth, saved.width));
  const height = Math.min(monitor.height, Math.max(minHeight, saved.height));
  return {
    width,
    height,
    x: Math.min(Math.max(saved.x, monitor.x), monitor.x + monitor.width - width),
    y: Math.min(Math.max(saved.y, monitor.y), monitor.y + monitor.height - height),
  };
}

function load(): WindowState | undefined {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    if (stored === null || typeof stored !== "object") return undefined;
    const { x, y, width, height, maximized } = stored as Record<string, unknown>;
    return [x, y, width, height].every((value) => typeof value === "number" && Number.isFinite(value))
      ? { x: x as number, y: y as number, width: width as number, height: height as number, maximized: maximized === true }
      : undefined;
  } catch {
    return undefined;
  }
}

function save(state: WindowState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // The next launch opens at the default size.
  }
}

/** Puts the window back where it was last closed, kept on a connected monitor. */
export async function restoreWindowState(): Promise<void> {
  const saved = load();
  if (!saved) return;
  const monitors = (await availableMonitors()).map((monitor) => ({
    x: monitor.position.x, y: monitor.position.y, width: monitor.size.width, height: monitor.size.height,
  }));
  const appWindow = getCurrentWindow();
  const scale = await appWindow.scaleFactor();
  const target = clampToMonitors(saved, monitors, MIN_WIDTH * scale, MIN_HEIGHT * scale);
  if (!target) return;
  await appWindow.setSize(new PhysicalSize(target.width, target.height));
  await appWindow.setPosition(new PhysicalPosition(target.x, target.y));
  if (saved.maximized) await appWindow.maximize();
}

/** Remembers the window size and position while it moves; a maximized window keeps its last normal size. */
export async function watchWindowState(): Promise<() => void> {
  const appWindow = getCurrentWindow();
  let timer: number | undefined;
  const capture = async () => {
    if (await appWindow.isMinimized()) return;
    if (await appWindow.isMaximized()) {
      const previous = load();
      if (previous) save({ ...previous, maximized: true });
      return;
    }
    const [position, size] = await Promise.all([appWindow.outerPosition(), appWindow.outerSize()]);
    save({ x: position.x, y: position.y, width: size.width, height: size.height, maximized: false });
  };
  const later = () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void capture().catch(() => {}), 500);
  };
  const stops = await Promise.all([appWindow.onResized(later), appWindow.onMoved(later)]);
  return () => {
    window.clearTimeout(timer);
    for (const stop of stops) stop();
  };
}
