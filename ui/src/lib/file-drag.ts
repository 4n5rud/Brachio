const THRESHOLD = 5;

export interface FileDrag {
  label: string;
  /** Pointer position in CSS pixels while dragging. */
  onMove: (x: number, y: number) => void;
  onDrop: (x: number, y: number) => void;
  onEnd: () => void;
}

/**
 * Starts a pointer-driven drag after a short move. The webview's own drag and drop is taken by the
 * window's file-drop handling, so this follows the pointer itself and shows a small label beside it.
 */
export function startFileDrag(down: PointerEvent, drag: FileDrag): void {
  if (down.button !== 0 || down.pointerType === "touch") return;
  const startX = down.clientX;
  const startY = down.clientY;
  let ghost: HTMLElement | undefined;

  const finish = () => {
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", up, true);
    window.removeEventListener("pointercancel", cancel, true);
    window.removeEventListener("keydown", key, true);
    ghost?.remove();
    if (ghost) {
      // The click that ends a drag must not open the row it ended on.
      const swallow = (event: Event) => event.stopPropagation();
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, true), 0);
    }
    drag.onEnd();
  };
  function move(event: PointerEvent) {
    if (!ghost) {
      if (Math.hypot(event.clientX - startX, event.clientY - startY) < THRESHOLD) return;
      ghost = document.createElement("div");
      ghost.className = "file-drag-ghost";
      ghost.textContent = drag.label;
      document.body.append(ghost);
    }
    ghost.style.transform = `translate(${event.clientX + 12}px, ${event.clientY + 12}px)`;
    drag.onMove(event.clientX, event.clientY);
  }
  function up(event: PointerEvent) {
    const dragged = ghost !== undefined;
    if (dragged) drag.onDrop(event.clientX, event.clientY);
    finish();
  }
  function cancel() { finish(); }
  function key(event: KeyboardEvent) {
    if (event.key === "Escape") { event.preventDefault(); finish(); }
  }
  window.addEventListener("pointermove", move, true);
  window.addEventListener("pointerup", up, true);
  window.addEventListener("pointercancel", cancel, true);
  window.addEventListener("keydown", key, true);
}
