const EVENT = "brachio-open-tab";

/** Asks the task's window to open a tab of the kind used last, for the shortcut. */
export const requestNewTab = (taskId: string) =>
  window.dispatchEvent(new CustomEvent<string>(EVENT, { detail: taskId }));

/** A file (a path inside the task) or a page (an address) to show in one of the task's tabs. */
export interface ContentTabRequest {
  taskId: string;
  kind: "file" | "browser";
  target: string;
}

const CONTENT_EVENT = "brachio-open-content-tab";

/** Asks the task's window to show a file or page in a tab, selecting the tab that already shows it. */
export const requestContentTab = (request: ContentTabRequest) =>
  window.dispatchEvent(new CustomEvent<ContentTabRequest>(CONTENT_EVENT, { detail: request }));

/** Calls `handler` for each file or page request until the returned function is called. */
export function onContentTabRequest(handler: (request: ContentTabRequest) => void): () => void {
  const listener = (event: Event) => handler((event as CustomEvent<ContentTabRequest>).detail);
  window.addEventListener(CONTENT_EVENT, listener);
  return () => window.removeEventListener(CONTENT_EVENT, listener);
}

/** Calls `handler` for each request until the returned function is called. */
export function onNewTabRequest(handler: (taskId: string) => void): () => void {
  const listener = (event: Event) => handler((event as CustomEvent<string>).detail);
  window.addEventListener(EVENT, listener);
  return () => window.removeEventListener(EVENT, listener);
}
