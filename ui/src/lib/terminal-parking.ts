/**
 * Each mounted terminal pane holds an xterm instance with its scrollback, so only the most
 * recently used tasks keep one. The others are parked: their program keeps running in the runtime
 * and selecting one reattaches it. Six covers a working set of tasks without a visible stall when
 * switching between them.
 */
export const MAX_MOUNTED_TERMINALS = 6;

/**
 * Which ids to draw now. A task that was parked draws a placeholder for one frame first, so
 * switching to it paints at once instead of stalling while its terminal is built.
 */
export function drawnTaskIds(mounted: readonly string[], waking: ReadonlySet<string>): string[] {
  return mounted.filter((id) => !waking.has(id));
}

/** Moves `id` to the front of the selection history. */
export function touchRecent(recent: readonly string[], id: string): string[] {
  return [id, ...recent.filter((other) => other !== id)];
}

/**
 * Which open tasks keep a mounted pane, in `open` order so the DOM does not reshuffle. The
 * `max` most recently selected stay; `pinned` ones never park (an in-app terminal would be
 * stopped by unmounting) and do not use up a slot.
 */
export function mountedTaskIds(
  open: readonly string[],
  recent: readonly string[],
  pinned: ReadonlySet<string>,
  max: number = MAX_MOUNTED_TERMINALS,
): string[] {
  const openSet = new Set(open);
  const kept = new Set<string>();
  for (const id of recent) {
    if (kept.size >= max) break;
    if (openSet.has(id) && !pinned.has(id)) kept.add(id);
  }
  return open.filter((id) => kept.has(id) || pinned.has(id));
}
