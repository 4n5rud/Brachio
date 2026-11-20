Brachio is a light desktop app for running many coding agents (Claude Code, Codex) side by side, each in its own git worktree. The interface has one job: show which agent needs you, let you answer fast, then get out of the way. It runs on macOS and Windows, in four color themes with light and dark sets, and has to look calm on a 14-inch laptop with eight agents running.

## Principles
1. **Readability first.** When a choice trades looks for legibility, legibility wins. Nothing in the UI is smaller than 12px (`caption`); the app base is 14px (`body`); the terminal is 13px (`terminal`).
2. **Need first.** Everything is ordered by what the person has to do: Needs you, Working, Ready for review, Done. The task list, the board and the status bar all use this order.
3. **Status never relies on color.** Every state has its own glyph shape and a word (see StatusBadge).
4. **Light means light.** Few layers, no blur, no shadows on panels, one animation. Brachio's own memory and CPU stay visible in the StatusBar.
5. **Lay it out, don't explain it.** Order, grouping and size carry meaning. Standing explanation text is not used, except the ApprovalBar hint, form hints and error messages.

## Rules from the combined guideline
The screens follow the combined design guideline (myWorkFlow and unslop-ui). Where its fallback numbers differ, these tokens win. The parts that shape Brachio most:
- No eyebrow labels, decorative numbering, slogans or explain-the-screen sentences. A confirm dialog asks a question, lists what is lost and has two buttons: Cancel and a verb.
- Weights stay between 400 and 600. Spacing stays on the 4px scale. No gradients, glows or colored shadows.
- Toasts confirm what just happened and leave after 3s; errors stay until dismissed. Loading shows nothing under 0.3s, then the working arc with words.
- Repeating motion only for loading: the working arc. Status glyphs and badges never pulse.
- No middle dots or arrows to join phrases in UI text; split them with layout.

## Writing
- English is the source language. Korean is the second locale; Korean strings are written as natural Korean, not translated word for word. Every string lives in the i18n catalog (`ui/src/i18n`), never inline.
- Sentence case everywhere: "Ready for review", "Open PR", "Delete worktree".
- Buttons are verbs that say what happens. Results are past tense ("Pull request opened"). Errors say what went wrong and how to fix it, without apologies: "No branch named mian. Pick one from the list."
- Address the person as "you" only when needed ("Needs you"). Agents are named by product name ("Claude Code wants to run a command").
- Real values over vague ones: "Crashed with exit code 101", "42 uncommitted lines".
- No emoji, no exclamation marks. Times are relative in lists ("now", "6m", "3h", "2d") and absolute in local time in tooltips.
- Join items with layout (rows, columns), not with chains of commas, slashes or arrows.

## Color
- Four color themes, each with a dark and a light set: **Mint** (default, charcoal neutrals), **Lilac** (violet-tinted neutrals), **Sand** (warm paper neutrals, teal accent) and **Contrast** (black and white, for low vision and bright rooms). They are the eight token themes, `mint-dark` to `contrast-light`. The person picks Mode (System, Light, Dark) and a theme in Settings, Appearance (ThemePicker).
- A theme changes surfaces, text, lines, the accent and the focus ring. Status, diff and danger colors keep their meaning in every theme.
- Surfaces step up in this order: `color-canvas` (app ground and sunk fields), `color-panel` (all chrome), `color-raised` (secondary controls, selection), `color-hover`, `color-press`.
- Text: `color-text`, `color-text-2`, `color-text-3`. All three hold 4.5:1 or more on canvas, panel, raised and hover in all eight sets.
- Lines: `color-line` is a decorative hairline; `color-line-strong` is for edges that must be seen (inputs, toggles, kbd, dialog edge) and holds 3:1 everywhere.
- The accent (`color-accent` with `color-accent-ink`) is a fill only: primary button, toggle on, progress, the chosen swatch. Accent text and links use `color-accent-text`; focus uses `color-focus`.
- The brand mint (`color-mint`) belongs to the mascot and the app icon. It never changes with the theme.
- Status colors are for status only: working blue, needs you amber, ready green, error red, neutral grey. Red means an error or a destructive action, nothing else. `color-blocked-bg` and `color-error-bg` tint a notice row, never a whole card.
- Don't tint cards or put icons in colored circles. Emphasis comes from a status edge, a glyph or a word.

## Type
- `sans` is Pretendard Variable, chosen for Korean and Latin in one face; `mono` is JetBrains Mono. Both ship inside the app.
- Interface styles: `title` 18/600, `heading` 15/600, `body` 14/400, `label` 13/500, `caption` 12/400, `group-label` 12/600. Code styles: `meta` 12 mono, `terminal` 13 mono at 1.5.
- Branches, paths, commands, counts and ages are always mono.
- Korean text breaks between words (`word-break: keep-all`).

## Space, size and layout
- A 4px base (`space-1` to `space-8`). Chrome insets are 14px on the left of panels and 18px in content headers.
- Hit targets are 24px minimum and 28px by default (`size-row`). Settings rows are 44px.
- Dialogs: confirm `size-dialog` 480px, new task 560px, quick open 640px, settings 820px. The inspector widens to `size-inspector-wide` 560px while a diff is side by side.
- Window: `size-titlebar` 40px, `size-statusbar` 24px. Body: sidebar `size-sidebar` 288px, main area, inspector `size-inspector` 360px (Changes, Files, QA, Activity). Both side panels can be hidden (`⌘B`, `⌘⌥B`; Windows `Ctrl+Shift+B`, `Ctrl+Shift+Alt+B`) and the choice is kept. Minimum window 1000 × 600.
- Board view: tiles in `minmax(320px, 1fr)` columns.

## Shape and depth
- Radii: `radius-xs` 4px (kbd, chips), `radius-sm` 6px (buttons), 7px for rows, fields and segmented tracks, `radius-md` 9px (tiles, toasts), `radius-lg` 12px (dialogs).
- Panels are separated by 1px `color-line` hairlines, never by shadow. Only layers above the app (dialogs, toasts, menus) take `shadow-overlay`.

## Motion
- The working arc is the only repeating animation (1.1s per turn). Status glyphs, badges and dots never pulse or bounce.
- Other motion is short and functional: 120ms for hover and toggles. Under `prefers-reduced-motion`, everything stops, the arc included.

## States
Every interactive component defines default, hover, pressed, focus, disabled and loading. Focus is always a 2px `color-focus` ring with a 2px offset. Disabled is opacity 0.45. Loading keeps the size and shows the working arc with words. Lists also define empty, one item, many items, long values and Korean text.

## Iconography
- Brachio's own 16px stroke icons: 1.5px stroke, round caps and joins, drawn on a 16px grid, in `currentColor` (see the Icons group). Sizes: 15px in buttons, 13px beside labels, 11px in dense meta lines.
- An icon that stands alone sits in an IconButton with an `aria-label`.
- Status uses the six glyphs in StatusBadge, never icons.

## Brand mark
- The mascot is a flat mint brachiosaurus (`color-mint`) with black eyes (`color-brand-charcoal`) and no mouth. It appears in two places only: the app icon (on a charcoal squircle) and the first-run EmptyState.
- Never redraw, outline, animate or recolor it, and never add a mouth or expressions.

## Platforms
- macOS: overlay title bar with the system traffic lights, 80px left inset. Shortcuts use `⌘`.
- Windows: Brachio draws the caption buttons on the right. Shortcuts use `Ctrl`.
- Mode follows the OS by default; the person can force Dark or Light and pick a color theme in Settings, Appearance.
