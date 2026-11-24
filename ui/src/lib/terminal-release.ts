import type { AgentState } from "./status";
import type { TerminalHandle } from "./terminal";

export type TerminalLeave = "stop" | "detach" | "park";

/** Applies the pane's leave policy to a handle; used on unmount and when an open resolves too late. */
export function releaseTerminal(
  handle: TerminalHandle | null,
  leave: TerminalLeave | undefined,
  onStatus?: (status: AgentState, message: string | null) => void,
): void {
  if (!handle) return;
  if (leave === "park" && onStatus) void handle.park(onStatus).catch(() => handle.close());
  else if (leave === "park" || leave === "detach") void handle.detach().catch(() => handle.close());
  else void handle.close();
}
