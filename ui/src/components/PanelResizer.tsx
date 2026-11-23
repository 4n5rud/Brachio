import { t } from "../i18n";
import type { ResizablePanel } from "../lib/panel-widths";
import "./PanelResizer.css";

const KEY_STEP = 16;

/** A drag handle on the inner edge of the sidebar or inspector; arrow keys resize it too. */
export function PanelResizer(props: {
  panel: ResizablePanel;
  /** Receives the wanted width; the owner clamps it. */
  onResize: (width: number) => void;
  onCommit: () => void;
}) {
  // The sidebar grows as the pointer moves right, the inspector as it moves left.
  const direction = () => (props.panel === "sidebar" ? 1 : -1);

  // Measured, because the inspector may be at its CSS-computed wide size.
  const measure = (handle: HTMLElement) => {
    const edge = handle.offsetLeft + handle.offsetWidth / 2;
    return props.panel === "sidebar" ? edge : (handle.parentElement?.clientWidth ?? 0) - edge;
  };

  const startDrag = (event: PointerEvent) => {
    if (event.button !== 0) return;
    const handle = event.currentTarget as HTMLElement;
    const startX = event.clientX;
    const startWidth = measure(handle);
    handle.setPointerCapture(event.pointerId);
    const move = (next: PointerEvent) => props.onResize(startWidth + (next.clientX - startX) * direction());
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
      props.onCommit();
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const sign = event.key === "ArrowRight" ? 1 : -1;
    props.onResize(measure(event.currentTarget as HTMLElement) + sign * direction() * KEY_STEP);
    props.onCommit();
  };

  return (
    <div
      class="panel-resizer"
      classList={{ "panel-resizer-sidebar": props.panel === "sidebar", "panel-resizer-inspector": props.panel === "inspector" }}
      role="separator"
      aria-orientation="vertical"
      aria-label={t(props.panel === "sidebar" ? "layout.resizeSidebar" : "layout.resizeInspector")}
      tabindex="0"
      onPointerDown={startDrag}
      onKeyDown={onKeyDown}
    />
  );
}

/**
 * The drag handle between side-by-side terminals, in the middle column of their grid. Arrow keys
 * move it too and a double-click puts it back in the middle.
 */
export function SplitResizer(props: {
  /** The left terminal's share of the room both terminals share, as shown now. */
  ratio: number;
  /** Receives the wanted share; the owner clamps it. */
  onResize: (ratio: number) => void;
  onCommit: () => void;
  onReset: () => void;
}) {
  const room = (handle: HTMLElement) => (handle.parentElement?.clientWidth ?? 0) - handle.offsetWidth;

  const startDrag = (event: PointerEvent) => {
    if (event.button !== 0) return;
    // Without this the press starts a text selection that runs across the terminals.
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    const left = handle.parentElement?.getBoundingClientRect().left ?? 0;
    const width = room(handle);
    if (width <= 0) return;
    handle.setPointerCapture(event.pointerId);
    // Keeps the resize cursor and blocks selection while the pointer is over a terminal.
    document.documentElement.classList.add("split-resizing");
    const move = (next: PointerEvent) => props.onResize((next.clientX - left - handle.offsetWidth / 2) / width);
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
      handle.removeEventListener("lostpointercapture", end);
      document.documentElement.classList.remove("split-resizing");
      props.onCommit();
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
    handle.addEventListener("lostpointercapture", end);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const width = room(event.currentTarget as HTMLElement);
    const step = width > 0 ? KEY_STEP / width : 0.02;
    props.onResize(props.ratio + (event.key === "ArrowRight" ? step : -step));
    props.onCommit();
  };

  return (
    <div
      class="panel-resizer panel-resizer-split"
      role="separator"
      aria-orientation="vertical"
      aria-label={t("layout.resizeSplit")}
      aria-valuenow={Math.round(props.ratio * 100)}
      aria-valuemin="0"
      aria-valuemax="100"
      tabindex="0"
      onPointerDown={startDrag}
      onKeyDown={onKeyDown}
      onDblClick={() => props.onReset()}
    />
  );
}
