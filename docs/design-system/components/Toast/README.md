# Toast

A short message that confirms an action finished or failed, shown bottom center above the status bar.

## Use
- Past tense for results ("Pull request opened"). Errors say what failed and what to do next.
- At most one action. It stays 3s, or until dismissed when it carries an action or an error. Error toasts offer Report problem.
- The StatusBadge glyph on the left. `color-raised`, `color-line-strong` border, `radius-md`, `shadow-overlay`.

## Don't
- Never use a toast for an agent's question: that is the ApprovalBar and the Needs you group.
