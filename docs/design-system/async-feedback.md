# Action feedback

Every user-triggered async action shows pending, then success or failure. No silent actions.

## Rule
- **Pending.** The control that was pressed shows it: `IconButton busy` spins a refresh icon (any other icon becomes the working arc); `Button busy` or `loading` swaps the icon for the working arc and may swap the label (`loadingLabel`). The control is disabled and carries `aria-busy`. Do not only disable it.
- **Minimum 400ms.** Once shown, pending stays at least 400ms (`MIN_PENDING_MS`) so a fast action still registers. `useAsyncAction` and `holdMinimum` in `ui/src/lib/async-action.ts` apply it.
- **Success.** Pick the lightest form that fits: a check on the icon for 1.5s (`IconButton done`), a status line such as "Updated just now", an inline note, or a 3s toast for actions with no visible result (copy, reveal).
- **Failure.** Inline error text where the action sits, or an error toast with a Retry action (`useAsyncAction(..., { failureToast: true })`). Errors stay until dismissed.
- **Destructive actions** ask first in a confirm dialog that lists what is lost.
- **Disabled** controls must have an obvious reason on screen (empty field, running task, conflicts note).

## Use
```ts
const reload = useAsyncAction(async () => { await refetch(); }, { failureToast: true });
<IconButton icon="refresh" label={t("inspector.refresh")} busy={reload.pending()} done={reload.done()} onClick={() => void reload.run()} />
```
State store actions that several views share (usage, PR status) hold the minimum inside the store, so every view shows the same pending state.
