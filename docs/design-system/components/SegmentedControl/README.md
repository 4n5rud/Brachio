# SegmentedControl

A row of two to four mutually exclusive options shown at once.

## Use
- The Tasks and Board switch in the title bar; theme and density in settings.
- Each option is a `button` with `aria-pressed`; the group has `role="group"` and an `aria-label`.
- Labels are short nouns. An icon is optional and goes before the label.

## Anatomy
Track `color-raised` with a 1px `color-line` border, radius 7px, padding 2px, gap 2px. Segments 24px tall, 10px side padding, `label` style. Selected: `color-hover` fill, `color-text`, inset 1px `color-line-strong`.

## Don't
- More than four options: use a select.
- It switches a state; it never runs an action.
