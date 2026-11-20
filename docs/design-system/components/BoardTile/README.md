# BoardTile

A task as a tile in the board view, with the last lines of its terminal, so the person can scan many agents at once.

## Anatomy
- `color-panel`, 1px `color-line`, radius `radius-md`. Board grid: columns `minmax(320px, 1fr)`, gap `space-3`, padding `space-4` 18px.
- Head: the title (`body` 600, one line) and a StatusBadge; a meta line with AgentChip, short branch and DiffStat.
- Preview: the last two to four terminal lines in `terminal` style on `color-term-bg`, at least 64px tall. Plain text, no live cursor.
- Foot: the age and one action: "Review request" (primary) when blocked, "Open" otherwise.
- A tile that needs the person gets a `color-status-blocked` border mixed 50% into `color-line`; an error tile does the same with `color-status-error`.
- Tiles follow the same group order as the task list.

## Lead and worker
A lead task that splits work shows its workers as tiles in a row under it, joined by a hairline. The lead tile holds the decision gate ("Merge 3 workers") once every worker is ready.

## Don't
- Don't stream the preview at full frame rate; refresh it at most twice a second.
