const EVENT = "brachio-open-tab";

/** Asks the task's window to open a tab of the kind used last, for the shortcut. */
export const requestNewTab = (taskId: string) =>
  window.dispatchEvent(new CustomEvent<string>(EVENT, { detail: taskId }));

/** A file (a path inside the task) or a page (an address) to show in one of the task's tabs. */
export interface ContentTabRequest {
  taskId: string;
  kind: "file" | "browser";
  target: string;
  /** A single click: the tab is replaced by the next single click until it is kept or edited. */
  temporary?: boolean;
}

const CONTENT_EVENT = "brachio-open-content-tab";

/**
 * Asks the task's window to show a file or page in a tab, selecting the tab that already shows it.
 * Only the task on screen answers: a caller selects the task first. A request that arrives while
 * a tab is being added or closed is dropped, as a second click on the same row would be.
 */
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
