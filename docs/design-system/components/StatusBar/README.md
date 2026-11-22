# StatusBar

The 24px bottom bar that shows what Brachio itself costs.

## Use
- Left: how many sessions are running, then Brachio's memory and CPU. Memory is the working set of the app and the terminal runtime, including the agent and shell processes inside it, summed; CPU is the same set as a share of all cores. Being light is the product's promise, so these numbers stay in view.
- Right: the repository's current branch and how far it is from its remote.
- `caption` text in `color-text-3`; values in `meta` `color-text-2`.
- The dot is `color-status-ready` while every agent is fine; it becomes the blocked diamond with a count when one needs the person.

## Status
Implemented. The bar also shows the agent usage summary, which this page does not detail.

## Don't
- No buttons here. Anything clickable belongs in the title bar.
