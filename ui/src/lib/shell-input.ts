const EVENT = "brachio-shell-input";

/** Types a line into the shell terminal with this key, for buttons outside the terminal pane. */
export const sendShellInput = (key: string, text: string) =>
  window.dispatchEvent(new CustomEvent(EVENT, { detail: { key, text } }));

/** Calls `handler` for each line typed for `key` until the returned function is called. */
export function onShellInput(key: string, handler: (text: string) => void): () => void {
  const listener = (event: Event) => {
    const detail = (event as CustomEvent<{ key: string; text: string }>).detail;
    if (detail.key === key) handler(detail.text);
  };
  window.addEventListener(EVENT, listener);
  return () => window.removeEventListener(EVENT, listener);
}
