import type { Terminal } from "@xterm/xterm";

/**
 * Stops xterm.js from answering the primary device attributes query (`ESC [ c`) of a terminal
 * whose host already answers it. ConPTY sends that query at startup and the runtime replies; a
 * second reply from the screen reaches the shell as typed input, which cmd prints as `^[[?1;2c`.
 */
export function silenceDeviceAttributes(term: Pick<Terminal, "parser">, hostAnswers: () => boolean) {
  return term.parser.registerCsiHandler({ final: "c" }, () => hostAnswers());
}
