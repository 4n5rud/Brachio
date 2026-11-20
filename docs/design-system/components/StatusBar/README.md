# StatusBar

The 24px bottom bar that shows what Brachio itself costs.

## Use
- Left: how many agents are running, then Brachio's own memory and CPU. Being light is the product's promise, so these numbers stay in view.
- Right: the repository's current branch and how far it is from its remote.
- `caption` text in `color-text-3`; values in `meta` `color-text-2`.
- The dot is `color-status-ready` while every agent is fine; it becomes the blocked diamond with a count when one needs the person.

## Don't
- No buttons here. Anything clickable belongs in the title bar.
