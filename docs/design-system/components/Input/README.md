# Input

A text field with its label above it, for the new task sheet and settings.

## Use
- Always a visible label (`label` style). The placeholder shows an example, never the label.
- Monospace (`input-mono`) for branches, paths and commands.
- A hint under the field in `caption` `color-text-3`. An error replaces the hint, in `color-status-error`, and says what is wrong and how to fix it.
- Multiline (`input-area`, 88px min) for the task prompt.

## Anatomy
32px min height, padding 7px 10px, radius 7px, `color-canvas` fill (sunk below the panel), 1px `color-line-strong` border.

## States
| State | Look |
|---|---|
| Default | as above |
| Focus | border `color-focus` plus a 3px focus ring at 22% |
| Error | border `color-status-error`, `aria-invalid="true"`, message below |
| Disabled | opacity 0.45 |

## Don't
- Don't validate while the person is still typing a first value; check on blur or submit.
