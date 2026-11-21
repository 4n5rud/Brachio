# Tooltip

A small label above an icon-only control that names the action. IconButton shows one automatically.

## Use
- Only to name a control that has no visible text. Never on a button that already shows its label, and never for content people must read or interact with.
- Appears after 600ms of hover or after keyboard focus (focus-visible); moving to a neighbouring control within 400ms shows the next one at once. Pressing, leaving, blurring or Esc hides it.
- A shortcut is shown as key caps (`Kbd`) after the label, never as text in parentheses.

## Anatomy
Above the control, 4px gap, centred. `color-panel` fill, 1px `color-line-strong` border, `radius-sm`, `shadow-overlay`, 12px text, one line, `pointer-events: none`.

## Don't
- Don't use the native `title` attribute beside it; two tooltips appear.
- Don't put a tooltip on a disabled control to explain why; say it next to the control.

## Consumer provides
`label`, optional `keys`, and a control that takes the trigger handlers.
