# Button

A text button for one clear action; primary marks the single most likely next step in a view.

## Use
- One `button-primary` per region at most: the task header, a dialog, the approval bar. Everything else is secondary (`button`) or `button-ghost`.
- `button-danger` only for actions that destroy work (delete a worktree, discard changes), and only inside a confirming Dialog. Red never appears for anything else.
- `button-sm` (26px) for dense spots: tile footers, the sidebar head. Default height is `size-row` (28px). Nothing smaller than 24px is clickable.
- Labels are verbs that say what happens: "Allow once", "Create PR", "Create task". No "OK", no "Submit".
- A shortcut hint goes inside the button as a Kbd, after the label.

- Quiet by default: toolbar, header, banner and notice actions use `button-ghost` (no border, no fill until hover), or an IconButton when the icon is unmistakable (archive, add tab). The bordered secondary and the filled primary are for dialogs and forms.
- Notice actions render ghost automatically; only a `button-primary` inside a Notice keeps its fill.

## Anatomy
Height `size-row`, side padding `space-3`, radius `radius-sm`, 1px `color-line` border, `label` style (13/500; primary 600). Optional 13px icon before the label, gap 6px.

## States
| State | Secondary | Primary |
|---|---|---|
| Default | `color-raised` fill | `color-accent` fill, `color-accent-ink` text |
| Hover | `color-hover` | accent at brightness 0.92 |
| Pressed | `color-press` | accent at brightness 0.86 |
| Focus | 2px `color-focus` ring, offset 2px | same |
| Disabled | opacity 0.45, no hover | same |
| Loading | `aria-busy="true"`; the working arc replaces the icon and the label says what is happening ("Creating…"); width stays the same | same |

## Don't
- Don't put two primary buttons side by side.
- Don't use the accent fill as text: `color-accent` is a fill, `color-accent-text` is the text color.
- Don't swap the label for an icon on narrow widths; shrink the region instead.

## Consumer provides
The label, an optional 16px stroke icon and an optional shortcut, on a native `<button>` with `class="button"` plus one modifier.
