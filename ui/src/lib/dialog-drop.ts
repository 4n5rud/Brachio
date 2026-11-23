let handler: ((paths: string[]) => void) | undefined;

/** The New task dialog takes files dropped on the window while it is open. */
export function registerDialogDrop(next: (paths: string[]) => void): () => void {
  handler = next;
  return () => {
    if (handler === next) handler = undefined;
  };
}

export const dropIntoDialog = (paths: string[]): boolean => {
  handler?.(paths);
  return handler !== undefined;
};
