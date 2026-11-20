# ConfirmDialog

A centered dialog before a step that loses something: delete a task for good, discard a draft, sign out of Codex, forget an unknown launch.

## Use
- The title is a question ("Delete this task for good?"). The body says what goes, then a loss list with real counts: uncommitted files, commits only on this branch, ignored items, running terminals. Counts above zero are `color-status-error`; zeros are muted.
- Two buttons: Cancel and the verb (`button-danger` when work is lost). No "OK", no "Yes". Focus starts on Cancel; `Esc` cancels; clicking the scrim does nothing.
- `size-dialog` 480px, `color-panel`, `color-line-strong`, `radius-lg`, `shadow-overlay`, over `color-scrim`.
- Archive uses the same dialog with a primary button, because nothing is lost.
