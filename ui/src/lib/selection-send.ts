import { createSignal } from "solid-js";

/** Where on screen the person asked to send a terminal selection. */
export interface SelectionRequest {
  taskId: string;
  /** The terminal the text was selected in; it is never offered as the destination. */
  sourceKey: string;
  text: string;
  x: number;
  y: number;
}

const [request, setRequest] = createSignal<SelectionRequest>();

/** The selection waiting for the person to choose an agent tab. */
export const selectionRequest = request;
export const requestSelectionSend = (next: SelectionRequest) => setRequest(next);
export const clearSelectionRequest = () => setRequest(undefined);

/** Quotes selected terminal text as a fenced block, with a fence longer than any run of backticks inside it. */
export function fencedSelection(text: string): string {
  const body = text.replace(/\r\n?/gu, "\n").replace(/\n+$/u, "");
  const longest = Math.max(0, ...(body.match(/`+/gu) ?? []).map((run) => run.length));
  const fence = "`".repeat(Math.max(3, longest + 1));
  return `${fence}\n${body}\n${fence}\n`;
}
