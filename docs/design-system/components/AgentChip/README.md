# AgentChip

A small outlined tag naming the agent that runs a task.

## Use
- In task rows, the task header and tiles.
- The text is the agent's product name exactly: "Claude Code", "Codex".
- 12px/600 `color-text-2`, 1px `color-line-strong` border, radius `radius-xs`, 6px side padding, 18px tall.
- Outlined, never filled: Brachio does not color-code agents.

The app code from PR #2 still sets this chip at 11px and the count badge at 10px. The floor is 12px; the code moves to this spec in the next UI change.
