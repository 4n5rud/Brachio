# Command palette, Go To and undo

## Palette modes
Quick open (shortcut in `lib/shortcut.ts`) is one mixed list of tasks, files and actions. Typing `>` first shows only actions; typing `@` first opens Go To.

## Add an action
Register it once from the code that owns the behavior; `registerActions` returns the remover, so call it inside `onCleanup`.

```ts
onCleanup(registerActions({
  id: "task.archive",              // unique and stable
  label: () => t("task.archive"),
  group: () => t("action.groupTask"),
  keys: "⌘⇧A",                     // optional chip; take it from SHORTCUTS
  available: () => selected() !== undefined,
  run: () => archive(selected()!),
}));
```
Actions live in `ui/src/lib/actions.ts`. Do not add a second list of commands to the palette.

## Go To
Rows are agent or terminal tabs grouped by project; Left/Right switch the project, Enter focuses the task and tab. Owners add rows with `registerGoToSource(() => GoToTarget[])`. The source is reactive. The app already registers one row per task; the terminal owner should register one row per terminal tab with `kind: "terminal"` and a `focus` that selects the task and the tab.

## Undo
`pushUndoToast(message, undo)` in `lib/undo.ts` shows "실행 취소" for 6 seconds. Use it only with a real inverse (unarchive, move back). Actions with no safe inverse (delete task) get no undo button. Wired: archive. Hooks for other owners: task delete has none by design; file move and project removal call `pushUndoToast` with their inverse once one exists.
