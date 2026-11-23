interface TailSource {
  read: () => string[];
}

const sources = new Map<string, TailSource>();

/** Lets other prompts read the last lines of a terminal, by its tab key. */
export function registerTerminalTail(key: string, read: () => string[]): () => void {
  const source = { read };
  sources.set(key, source);
  return () => {
    if (sources.get(key) === source) sources.delete(key);
  };
}

export const terminalTailKeys = (): string[] => [...sources.keys()];
export const readTerminalTail = (key: string): string[] | undefined => sources.get(key)?.read();
