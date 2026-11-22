export interface ShellDropTarget {
  insert: (text: string) => Promise<boolean>;
}

const shells = new WeakMap<Element, ShellDropTarget>();
const SELECTOR = ".task-view [data-agent-terminal], .task-view [data-shell-terminal]";

/** Lets a plain shell terminal receive dropped paths as typed input. */
export function registerShellDropTarget(host: Element, target: ShellDropTarget): () => void {
  shells.set(host, target);
  host.setAttribute("data-shell-terminal", "");
  return () => {
    shells.delete(host);
    host.removeAttribute("data-shell-terminal");
  };
}

export type TerminalDrop = { kind: "agent"; host: Element } | { kind: "shell"; host: Element; target: ShellDropTarget };

/** The terminal under a point given in CSS pixels. */
export function terminalAt(x: number, y: number): TerminalDrop | undefined {
  const host = document.elementFromPoint(x, y)?.closest(SELECTOR);
  if (!host) return undefined;
  const target = shells.get(host);
  return target ? { kind: "shell", host, target } : { kind: "agent", host };
}
