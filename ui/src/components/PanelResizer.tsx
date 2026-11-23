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
