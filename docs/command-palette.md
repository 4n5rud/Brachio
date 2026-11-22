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
Rows are agent or terminal tabs grouped by project; Left/Right switch the project, Enter focuses the task and tab. Add rows with `registerGoToSource(() => GoToTarget[])`; the source is reactive. The app registers one row per task, and the task view registers one row per terminal tab (`kind: "terminal"`, with a `focus` that selects the task and the tab).

## Undo
`pushUndoToast(message, undo)` in `lib/undo.ts` shows "실행 취소" for 6 seconds. Use it only with a real inverse (unarchive, move back). Actions with no safe inverse (delete task) get no undo button. Archive uses it. Task delete has no undo by design. A new action with a safe inverse should call `pushUndoToast` with that inverse.

## Project cues
A project keeps reusable prompts and shell commands (`project_cues` table, `lib/cues.ts`). They appear in the palette under "프로젝트 Cue" and in the task header menu. A prompt goes through the draft flow (typed into the agent input, never sent). A shell cue runs through the runner the terminal registers with `registerShellCueRunner((taskId, command) => Promise<boolean>)`; it types the command once the shell is ready and returns false when the task has no shell tab.
