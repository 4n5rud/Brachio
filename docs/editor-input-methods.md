# Input methods in the code editor

The file tab editor is CodeMirror 6 (`@codemirror/view` 6.43, `@codemirror/state` 6.7 when checked). This page records what was checked about composition (Korean, Japanese and Chinese input) before the file tab is built, and what still needs a person with a real input method on each web view. Nothing here was run with a real input method yet.

## Checked in the editor source

- On Chromium (WebView2 on Windows) the editor reads composition from the editable element's DOM changes and the `compositionstart`, `compositionupdate` and `compositionend` events. The newer `EditContext` input path is used only on Android, so WebView2 uses the same path as other desktop Chromium builds.
- A composition in progress is tracked as a state (`view.composing`, `view.compositionStarted`). The editor does not rebuild the line being composed while it is active, so an inline candidate is not torn down mid-syllable.
- With several cursors, the editor limits what a composition may replace so the extra cursors are not corrupted. It also keeps the composition range mapped when another change, such as an undo or an agent edit applied to the open file, lands during composition.
- Safari, and so WKWebView on macOS, has code for two known engine problems: a `keydown` that arrives right after `compositionend` is held back for 100 ms so Enter or Backspace is not applied twice, and a missing `compositionend` after a dead-key composition is synthesized after 20 ms.
- Backspace inside a composition is handled by the browser's composition and the editor's DOM read, not by a key binding. Do not bind Backspace in the editor keymap without testing it with Korean input.

## Rules the file tab follows

- Key bindings that act on printable keys or on Enter, Backspace, Space and the arrow keys check `view.composing` first and do nothing while it is true. This covers bracket auto-close, auto-indent, list continuation in markdown mode, and any tab-level shortcut.
- The tab's own shortcuts (save, close, switch tab) use modifier keys and are not run while a composition is open.
- A document change from outside the editor (a reload after an agent edit) is not applied while `view.composing` is true. It is applied after the composition ends.
- No decoration may replace or hide the text of the line being composed. Live preview in markdown mode keeps the active line as source.

## Manual checks (not done)

Run each on Windows with WebView2 and on macOS with WKWebView, using a fresh editor with a plain text file. macOS could not be run while this was written.

Korean (2-set keyboard):
1. Type `한글입력` in one line. Each syllable stays whole while typing.
2. Type `ㅎ`, `ㅏ`, `ㄴ`, then Backspace three times. The jamo are removed one at a time, and no stray character is left.
3. Type `ㅎㅏㄴ`, then commit with Space, Enter, the Right and Left arrow keys, and a mouse click elsewhere in the line. Each leaves `한` in place and moves as expected, with no doubled or lost character.
4. Type `한` then press Ctrl+Z (Cmd+Z) and Ctrl+Y (Cmd+Shift+Z). Text is restored without a broken syllable.
5. With two cursors (Alt+click), type `가나`. Both cursors receive the text.
6. Type `(` then `한`. The bracket closes, `한` ends up inside it, and typing `)` steps over the closing bracket.

Japanese (Microsoft IME on Windows, Hiragana on macOS) and Chinese (Pinyin):
1. Type `nihon` and then Space. The candidate window appears next to the cursor and not at a screen corner. Scroll the editor so the cursor is near the bottom of the window and check it again.
2. Choose a candidate with Enter and the number keys. Text commits once.
3. Repeat step 1 with the cursor at the end of a long line that scrolls sideways, and in a line with a fold.
4. Undo and redo after committing. Text is restored without a half-converted string.
5. With two cursors, convert and commit. Both cursors receive the text.

Report any failure with the OS, IME, web view version and the step number.
