const EVENT = "brachio-open-shell";

/** Asks the window to show the task's shell tab, for buttons outside the terminal area. */
export const requestShell = (taskId: string) =>
  window.dispatchEvent(new CustomEvent<string>(EVENT, { detail: taskId }));

/** Calls `handler` for each request until the returned function is called. */
export function onShellRequest(handler: (taskId: string) => void): () => void {
  const listener = (event: Event) => handler((event as CustomEvent<string>).detail);
  window.addEventListener(EVENT, listener);
  return () => window.removeEventListener(EVENT, listener);
}
