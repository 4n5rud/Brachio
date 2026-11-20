export interface ApprovalKeyContext {
  key: string;
  /** Ctrl, Meta, Alt or Shift is held. */
  modified: boolean;
  composing: boolean;
  /** Keyboard focus is inside the approval bar. A key typed anywhere else, the terminal included, never answers. */
  inBar: boolean;
  /** Focus is on a button, link or tab, where Enter already means "activate". */
  onControl: boolean;
  dialogOpen: boolean;
}

/** Enter allows and Escape denies, only while focus is inside the approval bar. */
export function approvalKeyAction(context: ApprovalKeyContext): "allow" | "deny" | null {
  if (!context.inBar || context.modified || context.composing || context.dialogOpen) return null;
  if (context.key === "Escape") return "deny";
  if (context.key === "Enter" && !context.onControl) return "allow";
  return null;
}
