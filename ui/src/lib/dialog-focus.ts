const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

export function focusableIn(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((node) => node.getClientRects().length > 0);
}

/** Index Tab or Shift+Tab lands on, wrapping at both ends; -1 means the dialog itself. */
export function wrapFocusIndex(count: number, current: number, backwards: boolean): number {
  if (count === 0) return -1;
  if (current < 0) return backwards ? count - 1 : 0;
  return (current + (backwards ? -1 : 1) + count) % count;
}

// Only the top-most dialog reacts to Esc, so a confirm over a form closes alone.
const open: symbol[] = [];

export function registerDialog(): { isTop: () => boolean; release: () => void } {
  const id = Symbol("dialog");
  open.push(id);
  return {
    isTop: () => open[open.length - 1] === id,
    release: () => {
      const at = open.indexOf(id);
      if (at >= 0) open.splice(at, 1);
    },
  };
}
