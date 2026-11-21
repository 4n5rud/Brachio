# Menu

A list of actions opened from a button. It closes on Esc, an outside click or choosing a row.

## Use
- Rows: icon, label, then on the right a quiet `hint` (what the row opens) and/or shortcut key caps. These are separate: a hint is words, a shortcut is `Kbd` chips, and a row may show both.
- Arrow Up and Down move through the rows and wrap; focus starts on the first row.
- A submenu row (`MenuSubmenu`) ends in a right chevron and `aria-haspopup="menu"`. Hover, Enter, Space, a click or Right arrow opens it to the right and focuses its first row. Left arrow or Esc closes it and returns to the row; Esc on the top level closes the menu and focuses the button.
- Danger text only for rows that delete something.

## Anatomy
Rows are `size-row` (28px) tall with 10px side padding and 6px radius; the panel has 4px padding, a hairline border and `shadow-overlay`. The submenu overlaps the parent by 2px so the pointer never crosses a gap.

## Don't
- Don't put shortcuts into the hint text.
- Don't nest a submenu inside a submenu.
