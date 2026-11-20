# TaskHeader

The two-line header above a task's terminals, with its tab strip.

## Anatomy
- Height `size-task-header` (60px), `color-panel`, bottom hairline. Insets 18px left, 14px right.
- Line 1: the task title in `heading` (15/600), one line.
- Line 2: StatusBadge, AgentChip, the full branch with the branch icon, DiffStat, in `meta`.
- Actions on the right. The primary follows the state: "Open PR" is primary once the task is ready; while working or blocked nothing here is primary (when blocked, the ApprovalBar holds the decision).
- Tabs below at `size-tabs` (34px): one per terminal, the agent's first. The selected tab takes `color-term-bg` so it joins the pane under it.

## Don't
- Don't squeeze the branch onto line 1; two lines keep both readable.
