import { createSignal } from "solid-js";

/** Where files dropped from the OS go: the Files panel registers while it is showing a task. */
export interface TreeDropHandler {
  /** Folder the dropped files land in, from the pointer's folder row, or `undefined` outside the tree. */
  copyInto: (dir: string, paths: string[]) => Promise<void>;
}

let handler: TreeDropHandler | undefined;
const [osDropDir, setOsDropDir] = createSignal<string>();
export { osDropDir };

export function registerTreeDrop(next: TreeDropHandler): () => void {
  handler = next;
  return () => {
    if (handler === next) handler = undefined;
    setOsDropDir(undefined);
  };
}

/** The tree folder under a point given in CSS pixels; the empty string is the task root. */
export function treeDirAt(x: number, y: number, tree?: Element): string | undefined {
  const spot = document.elementFromPoint(x, y)?.closest("[data-drop-dir]");
  if (!spot || (tree && !tree.contains(spot))) return undefined;
  return spot.getAttribute("data-drop-dir") ?? undefined;
}

/** Whether the point is on the Files tree, which takes OS drops. */
export function overTree(x: number, y: number): boolean {
  return handler !== undefined && treeDirAt(x, y) !== undefined;
}

/** Marks the folder an OS drag is over. */
export function hoverOsDrop(x: number, y: number) {
  setOsDropDir(handler ? treeDirAt(x, y) : undefined);
}

export function clearOsDrop() { setOsDropDir(undefined); }

/** Hands files dropped on the tree to the panel; false when the drop was not on the tree. */
export function dropOnTree(x: number, y: number, paths: string[]): boolean {
  const dir = handler ? treeDirAt(x, y) : undefined;
  setOsDropDir(undefined);
  if (dir === undefined || !handler) return false;
  void handler.copyInto(dir, paths);
  return true;
}
