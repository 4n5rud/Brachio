/** What the interval needs to know about the window. */
export interface PageVisibility {
  hidden(): boolean;
  /** Calls `onChange` when the window is hidden, shown or focused; returns the unsubscribe. */
  listen(onChange: () => void): () => void;
}

const browserPage: PageVisibility = {
  hidden: () => document.visibilityState === "hidden",
  listen: (onChange) => {
    document.addEventListener("visibilitychange", onChange);
    window.addEventListener("focus", onChange);
    return () => {
      document.removeEventListener("visibilitychange", onChange);
      window.removeEventListener("focus", onChange);
    };
  },
};

/**
 * Like `setInterval`, but idle while the window is hidden. When it becomes visible again (or
 * regains focus after being hidden) `callback` runs once at once and the interval restarts.
 * Returns the function that stops it.
 */
export function visibleInterval(callback: () => void, ms: number, page: PageVisibility = browserPage): () => void {
  let timer: ReturnType<typeof setInterval> | undefined;
  const start = () => {
    timer = setInterval(callback, ms);
  };
  const stop = () => {
    if (timer !== undefined) clearInterval(timer);
    timer = undefined;
  };
  if (!page.hidden()) start();
  const unlisten = page.listen(() => {
    if (page.hidden()) {
      stop();
    } else if (timer === undefined) {
      callback();
      start();
    }
  });
  return () => {
    stop();
    unlisten();
  };
}
