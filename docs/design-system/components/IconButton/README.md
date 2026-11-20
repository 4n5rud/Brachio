# IconButton

A square button holding one icon, for chrome actions people repeat often.

## Use
- Title bar, pane heads, tile heads: view switch, split, settings, notifications, more.
- Always set `aria-label` to the action ("Split pane") and show the same words as a tooltip after 500ms.
- 28px (`size-row`) by default, 24px (`icon-button-sm`) in pane heads. Never smaller.
- A count badge sits top right only for things waiting on the person (tasks that need you): `color-status-blocked` fill, `color-badge-ink` digits, 12px mono, 16px tall.

## States
Default transparent with a `color-text-2` icon. Hover `color-hover`. Focus ring `color-focus`. Disabled opacity 0.45.

## Don't
- Don't use it for an action people would not recognize from the icon alone; use a Button with a label.
- Don't put icons inside tinted circles.
