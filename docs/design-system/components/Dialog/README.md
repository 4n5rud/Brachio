# Dialog

A modal that asks for confirmation before a step that cannot be undone.

## Use
- Only for destructive or irreversible steps: delete a worktree, discard changes, force push.
- The title is the question. The body names exactly what will be lost, with real names and counts.
- Buttons: Cancel (ghost) and the action named by its verb ("Delete worktree") as `button-danger`. Focus starts on Cancel; `Esc` cancels.
- `color-panel`, `color-line-strong` border, `radius-lg`, `shadow-overlay`, `size-dialog` 480px wide, over `color-scrim`.
- The new task sheet uses the same shell at 560px, settings at 820px, quick open at 640px. See ConfirmDialog for the loss list.
