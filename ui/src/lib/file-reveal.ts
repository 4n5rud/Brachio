import { createSignal } from "solid-js";

export interface RevealRequest {
  taskId: string;
  path: string;
  /** Distinguishes two requests for the same file. */
  serial: number;
}

const [request, setRequest] = createSignal<RevealRequest>();
let serial = 0;

/** The last request to show a file in the Files tree; the Files panel follows it. */
export const revealRequest = request;

export function revealInTree(taskId: string, path: string): void {
  setRequest({ taskId, path, serial: ++serial });
}
