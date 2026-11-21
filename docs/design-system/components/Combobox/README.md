# Combobox

An input with a chevron on its right edge and a list of choices under it. Used for the base branch and the project picker.

## Use
- When the person can pick from a known list but may also type a value that is not on it (a commit, a tag). For a fixed list with nothing else allowed, use a native Select.
- The consumer filters: it passes the rows to show. Rows are `{ value, label, detail?, icon? }`; `detail` is muted text after the label (where a branch lives, an owner/repo).
- A visible label above, like Input. The list has its own name (`listLabel`) shown as its heading.
- The list opens on focus, typing, the chevron or Arrow Down. Up and Down move and wrap, Enter picks, Esc closes.

## Anatomy
The input shell is an Input (32px, or 28px with `compact`); a 20px ghost chevron sits 4px from the right edge. The chevron rotates 180 degrees while the list is open and never takes focus: it has `tabindex="-1"` and ignores mousedown so the caret stays in the input. The list is a Menu surface at least as wide as the field, at most 288px tall, and opaque.

## States
| State | Look |
|---|---|
| Closed | input plus chevron pointing down |
| Open | chevron pointing up, active row filled `color-hover` |
| Focus | the Input focus ring |
| Error | `aria-invalid="true"`, border `color-status-error`, message below |
| Disabled | opacity 0.45, chevron inert |

## Don't
- Don't use a chevron on a field that has no list.
- Don't hide the typed text behind a chosen label; the field shows the value as typed.

## Consumer provides
`value`, `onValue`, the filtered `options`, `listLabel`, and optionally `onPick`.

## Footer and two-line rows
- `footer` pins a row under a divider below the scrolling rows, usually a `MenuSubmenu` such as "Add new project" with its three ways in. Arrow Down from the last row focuses it; Arrow Up or Esc returns to the input.
- `subline` puts a muted second line under a row's label: owner/repo for a project, or its path when it has no remote.
- A native Select gets the same chevron (not rotating).
