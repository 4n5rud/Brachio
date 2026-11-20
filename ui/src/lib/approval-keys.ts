export interface ApprovalKeyContext {
  key: string;
  /** Ctrl, Meta, Alt or Shift is held. */
  modified: boolean;
  composing: boolean;
  inTerminal: boolean;
  /** Focus is in a text field outside the terminal. */
  inField: boolean;
  /** Focus is on a button, link or tab, where Enter already means "activate". */
  onControl: boolean;
  dialogOpen: boolean;
}

/** Enter allows and Escape denies, also while the terminal has focus; any other focus keeps its own meaning for the key. */
export function approvalKeyAction(context: ApprovalKeyContext): "allow" | "deny" | null {
  if (context.modified || context.composing || context.dialogOpen) return null;
  if (context.inField && !context.inTerminal) return null;
  if (context.key === "Escape") return "deny";
  if (context.key === "Enter" && (context.inTerminal || !context.onControl)) return "allow";
  return null;
}
