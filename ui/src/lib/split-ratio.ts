const SPLIT_KEY = "brachio.splitRatio";

/** Side-by-side terminals start, and reset on double-click, at an even split. */
export const DEFAULT_SPLIT_RATIO = 0.5;
/** Width of the drag handle between the two terminals; it takes the place of the grid gap. */
export const SPLIT_HANDLE_PX = 8;
/** Each terminal keeps at least this share of the row, and at least this many pixels. */
const MIN_SPLIT_SHARE = 0.2;
const MIN_SPLIT_PX = 240;

/**
 * Fits a stored or dragged ratio (the left terminal's share of `width`, the room both terminals
 * share) so that neither terminal gets narrower than its minimum. When the row is too narrow for
 * two minimums, both get half.
 */
export function clampSplitRatio(ratio: number, width: number): number {
  if (!Number.isFinite(ratio)) return DEFAULT_SPLIT_RATIO;
  const min = width > 0 ? Math.max(MIN_SPLIT_SHARE, MIN_SPLIT_PX / width) : MIN_SPLIT_SHARE;
  if (min >= 0.5) return DEFAULT_SPLIT_RATIO;
  const clamped = Math.min(1 - min, Math.max(min, ratio));
  return Math.round(clamped * 1000) / 1000;
}

export function loadSplitRatio(): number {
  try {
    const raw = localStorage.getItem(SPLIT_KEY);
    const value = raw === null ? Number.NaN : Number(raw);
    return Number.isFinite(value) && value > 0 && value < 1 ? value : DEFAULT_SPLIT_RATIO;
  } catch {
    return DEFAULT_SPLIT_RATIO;
  }
}

export function saveSplitRatio(ratio: number): void {
  try {
    localStorage.setItem(SPLIT_KEY, String(ratio));
  } catch {
    // The ratio still applies for this run.
  }
}
