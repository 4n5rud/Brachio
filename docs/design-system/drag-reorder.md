# Drag to reorder

One primitive, `ui/src/lib/drag-reorder.ts` (`createReorder`), moves the sidebar projects, the tasks inside a project and the extra tabs of a task. It uses pointer events, not HTML5 drag and drop, so it behaves the same in WebView2 and WKWebView. The stylesheet is `ui/src/components/ui/Reorder.css`.

## Use
- Call `createReorder({ axis, ids, container, label, onReorder })` once per list and spread `reorder.item(id)` on each item. `ids` is the current order; `onReorder` receives the full new order.
- A press on the whole item drags it, or only a part of it when `handle` names a selector (project cards drag by their header). Mark inner controls (import, add, close) with `data-no-drag`.
- Nested lists each get their own scope, so a task list inside a project card never picks up the cards.
- `placeInOrder` puts a subset of a list in the new order inside the slots it already holds; use it to apply `onReorder` to state.

## Behavior
- A press becomes a drag after 5px of travel. Below that it is a click, and the click that ends a drag is swallowed.
- The dragged item dims. A 2px `color-focus` line on the edge of the target item shows where it lands.
- A pointer outside the list is a no-drop: the item dims further and the cursor is `not-allowed`. This is how a task is refused a move into another project.
- Near the edge of the `scroller`, the list scrolls (up to 18px a frame).
- Esc, losing window focus or a pointer cancel puts everything back.
- Keyboard: Alt+Up or Alt+Down (Alt+Left or Alt+Right for tabs) on a focused item moves it one place and keeps focus on it.
- A polite live region announces picked up, moved to position N of M, and canceled. Strings are `reorder.*` in the i18n catalog.

## Persistence
Each list saves the whole new order on drop: `repo_reorder`, `task_reorder` and `task_reorder_tabs`. Rows never dragged keep their old order; new projects and tabs sort last and new tasks first.
