const EVENT = "brachio-open-tab";

/** Asks the task's window to open a tab of the kind used last, for the shortcut. */
export const requestNewTab = (taskId: string) =>
  window.dispatchEvent(new CustomEvent<string>(EVENT, { detail: taskId }));

/** Calls `handler` for each request until the returned function is called. */
export function onNewTabRequest(handler: (taskId: string) => void): () => void {
  const listener = (event: Event) => handler((event as CustomEvent<string>).detail);
  window.addEventListener(EVENT, listener);
  return () => window.removeEventListener(EVENT, listener);
}
