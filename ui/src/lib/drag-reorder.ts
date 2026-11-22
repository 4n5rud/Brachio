import { createSignal, onCleanup } from "solid-js";
import { t, type MessageKey } from "../i18n";

/** Pixels the pointer must travel before a press becomes a drag, so plain clicks still work. */
export const DRAG_THRESHOLD = 5;
const EDGE_ZONE = 36;
const MAX_SCROLL_STEP = 18;

export type Axis = "x" | "y";

/** The list after moving `from` to `to`; out-of-range requests return the list unchanged. */
export function moveItem<T>(items: readonly T[], from: number, to: number): T[] {
  const next = [...items];
  if (from < 0 || from >= next.length || to < 0 || to >= next.length || from === to) return next;
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved as T);
  return next;
}

/** Puts the listed items in the given order, in the slots they already occupy; other items stay where they are. */
export function placeInOrder<T extends { id: string }>(list: readonly T[], ids: readonly string[]): T[] {
  const byId = new Map(list.map((item) => [item.id, item]));
  const ordered = ids.flatMap((id) => byId.get(id) ?? []);
  const listed = new Set(ordered.map((item) => item.id));
  let next = 0;
  return list.map((item) => (listed.has(item.id) ? (ordered[next++] as T) : item));
}

/** Where the dragged item would land: the index of the first item whose midpoint is past the pointer. */
export function insertionIndex(spans: readonly { start: number; end: number }[], pointer: number): number {
  const index = spans.findIndex((span) => pointer < (span.start + span.end) / 2);
  return index === -1 ? spans.length : index;
}

/** Scroll distance for one frame: zero in the middle, growing toward the edge, signed by direction. */
export function autoScrollStep(pointer: number, start: number, end: number): number {
  if (pointer < start + EDGE_ZONE) return -Math.ceil(MAX_SCROLL_STEP * Math.min(1, (start + EDGE_ZONE - pointer) / EDGE_ZONE));
  if (pointer > end - EDGE_ZONE) return Math.ceil(MAX_SCROLL_STEP * Math.min(1, (pointer - (end - EDGE_ZONE)) / EDGE_ZONE));
  return 0;
}

let liveRegion: HTMLElement | undefined;

/** Speaks a message to screen readers through one shared polite live region. */
export function announce(message: string): void {
  if (typeof document === "undefined") return;
  if (!liveRegion || !liveRegion.isConnected) {
    liveRegion = document.createElement("div");
    liveRegion.className = "reorder-live";
    liveRegion.setAttribute("aria-live", "polite");
    liveRegion.setAttribute("role", "status");
    document.body.append(liveRegion);
  }
  liveRegion.textContent = "";
  // Writing in the next frame makes a repeated message read out again.
  requestAnimationFrame(() => { if (liveRegion) liveRegion.textContent = message; });
}

const fill = (key: MessageKey, values: Record<string, string | number>) =>
  Object.entries(values).reduce((text, [name, value]) => text.replace(`{${name}}`, String(value)), t(key));

export interface ReorderOptions {
  axis: Axis;
  /** Ids in their current order. */
  ids: () => readonly string[];
  /** The element that holds the items; a pointer outside it is a no-drop. */
  container: () => HTMLElement | undefined;
  /** The element that scrolls when the pointer nears its edge. */
  scroller?: () => HTMLElement | undefined;
  /** Presses only start a drag inside this selector; the whole item when left out. */
  handle?: string;
  /** Name read out for an item. */
  label: (id: string) => string;
  /** Called with the full new order once a drop or keyboard move settles. */
  onReorder: (ids: string[]) => void;
}

export interface Reorder {
  /** Handlers and attributes to spread on each item. Mark inner controls with `data-no-drag`. */
  item: (id: string) => Record<string, unknown>;
}

let scopes = 0;

/** Pointer-based drag to reorder, with Alt+arrow keyboard moves. Works the same in every webview. */
export function createReorder(options: ReorderOptions): Reorder {
  const [dragging, setDragging] = createSignal<string | undefined>(undefined);
  const [target, setTarget] = createSignal<{ id: string; edge: "before" | "after" } | undefined>(undefined);
  const [blocked, setBlocked] = createSignal(false);
  // Nested lists each have their own scope so one never picks up the other's items.
  const scope = String(++scopes);
  let stop: (() => void) | undefined;
  onCleanup(() => stop?.());

  const items = () => [...(options.container()?.querySelectorAll<HTMLElement>(`[data-reorder-scope="${scope}"]`) ?? [])];
  const commit = (id: string, to: number) => {
    const ids = options.ids();
    if (ids.indexOf(id) === -1 || to === ids.indexOf(id)) return false;
    options.onReorder(moveItem(ids, ids.indexOf(id), to));
    announce(fill("reorder.moved", { item: options.label(id), position: to + 1, total: ids.length }));
    return true;
  };

  const begin = (id: string, event: PointerEvent) => {
    if (event.button !== 0 || !event.isPrimary) return;
    const pressed = event.target as HTMLElement;
    if (pressed.closest("[data-no-drag]")) return;
    if (options.handle && !pressed.closest(options.handle)) return;
    const startX = event.clientX;
    const startY = event.clientY;
    let active = false;
    let pointerX = startX;
    let pointerY = startY;
    let frame = 0;
    const vertical = options.axis === "y";

    const place = () => {
      const container = options.container();
      if (!container) return;
      const box = container.getBoundingClientRect();
      const inside = pointerX >= box.left && pointerX <= box.right && pointerY >= box.top && pointerY <= box.bottom;
      setBlocked(!inside);
      if (!inside) { setTarget(undefined); return; }
      const rows = items();
      const spans = rows.map((row) => {
        const rect = row.getBoundingClientRect();
        return vertical ? { start: rect.top, end: rect.bottom } : { start: rect.left, end: rect.right };
      });
      const index = insertionIndex(spans, vertical ? pointerY : pointerX);
      const row = rows[index === rows.length ? index - 1 : index];
      const targetId = row?.dataset.reorderId;
      setTarget(targetId ? { id: targetId, edge: index === rows.length ? "after" : "before" } : undefined);
    };
    const tick = () => {
      const scroller = options.scroller?.();
      if (scroller) {
        const box = scroller.getBoundingClientRect();
        const step = vertical ? autoScrollStep(pointerY, box.top, box.bottom) : autoScrollStep(pointerX, box.left, box.right);
        if (step !== 0) {
          if (vertical) scroller.scrollTop += step; else scroller.scrollLeft += step;
          place();
        }
      }
      frame = requestAnimationFrame(tick);
    };
    const finish = (drop: boolean) => {
      cancelAnimationFrame(frame);
      document.removeEventListener("pointermove", move, true);
      document.removeEventListener("pointerup", up, true);
      document.removeEventListener("pointercancel", cancel, true);
      document.removeEventListener("keydown", key, true);
      window.removeEventListener("blur", cancel);
      document.documentElement.removeAttribute("data-reordering");
      stop = undefined;
      if (active) {
        // The click that ends a drag must not select or open the row.
        const swallow = (click: Event) => { click.stopPropagation(); click.preventDefault(); };
        window.addEventListener("click", swallow, { capture: true, once: true });
        setTimeout(() => window.removeEventListener("click", swallow, true), 0);
      }
      const hit = target();
      if (active && drop && hit) {
        const ids = options.ids();
        const from = ids.indexOf(id);
        let to = hit.edge === "before" ? ids.indexOf(hit.id) : ids.indexOf(hit.id) + 1;
        if (from < to) to -= 1;
        commit(id, to);
      } else if (active && !drop) {
        announce(t("reorder.canceled"));
      }
      setDragging(undefined);
      setTarget(undefined);
      setBlocked(false);
    };
    const move = (e: PointerEvent) => {
      pointerX = e.clientX;
      pointerY = e.clientY;
      if (!active) {
        if (Math.hypot(pointerX - startX, pointerY - startY) < DRAG_THRESHOLD) return;
        active = true;
        setDragging(id);
        document.documentElement.setAttribute("data-reordering", "");
        announce(fill("reorder.picked", { item: options.label(id) }));
        frame = requestAnimationFrame(tick);
      }
      e.preventDefault();
      place();
    };
    const up = () => finish(true);
    const cancel = () => finish(false);
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      finish(false);
    };
    document.addEventListener("pointermove", move, true);
    document.addEventListener("pointerup", up, true);
    document.addEventListener("pointercancel", cancel, true);
    document.addEventListener("keydown", key, true);
    window.addEventListener("blur", cancel);
    stop = () => finish(false);
  };

  const onKeyDown = (id: string, event: KeyboardEvent) => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const back = options.axis === "y" ? "ArrowUp" : "ArrowLeft";
    const forward = options.axis === "y" ? "ArrowDown" : "ArrowRight";
    if (event.key !== back && event.key !== forward) return;
    event.preventDefault();
    event.stopPropagation();
    const ids = options.ids();
    const to = Math.max(0, Math.min(ids.length - 1, ids.indexOf(id) + (event.key === forward ? 1 : -1)));
    if (commit(id, to)) {
      requestAnimationFrame(() => options.container()?.querySelector<HTMLElement>(`[data-reorder-scope="${scope}"][data-reorder-id="${CSS.escape(id)}"]`)?.focus());
    }
  };

  return {
    item: (id) => ({
      "data-reorder-id": id,
      "data-reorder-scope": scope,
      "data-reorder-axis": options.axis,
      get "data-dragging"() { return dragging() === id ? (blocked() ? "no-drop" : "true") : undefined; },
      get "data-drop"() { const hit = target(); return hit?.id === id && dragging() !== undefined ? hit.edge : undefined; },
      onPointerDown: (event: PointerEvent) => begin(id, event),
      onKeyDown: (event: KeyboardEvent) => onKeyDown(id, event),
    }),
  };
}
