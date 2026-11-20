# Toggle

A switch for a setting that takes effect at once.

## Use
- In settings rows: the name on the left with one line of consequence under it in `caption`, the switch on the right.
- `role="switch"` with `aria-checked`. The whole row is the hit target, at least 44px tall.
- Off: `color-line-strong` track, `color-text` knob. On: `color-mint` track, `color-mint-ink` knob. The knob slides in 120ms, with no slide under reduced motion.

## Don't
- Don't use a toggle for anything that needs Save; use a checkbox in a form.
