# TaskRow

One task in the sidebar list: status, title and age, then agent, branch and diff stat, with an optional note line.

## Grouping
The list groups tasks by what the person has to do, in this order: **Needs you**, **Working**, **Ready for review**, **Done**. Inside a group, newest activity first. Done starts collapsed; an empty group is hidden. The Needs you head is `color-status-blocked`; the others `color-text-3`. Heads use `group-label` with a mono count.

## Anatomy
A grid of a 16px glyph column, the title and the age. Padding 6px 8px, radius 8px, inset 6px from the sidebar edge. Title `body` at 500, one line with ellipsis. Age in `meta` `color-text-3` ("now", "6m", "3h", "2d"). Meta line: AgentChip, the branch without its `brachio/` prefix, DiffStat. An extra line holds the PrChip and the unread message count when there are any. Children (workers of a lead task) indent 24px under their lead with a connector. Optional note line at 12px `color-text-2`; blocked notes in `color-status-blocked`, errors in `color-status-error`.

## States and edge cases
| Case | Look |
|---|---|
| Default | transparent |
| Hover | `color-hover` |
| Selected | `color-raised` with an inset 1px `color-line` |
| Focus | `color-focus` ring |
| Long title | one line with ellipsis; the full title is in the header and a tooltip |
| Korean title | breaks between words (`word-break: keep-all`); the branch falls back to `task-<id>` |
| No changes yet | DiffStat hidden |
| No tasks | the sidebar says "No tasks yet" and shows the new task shortcut |
| 200+ tasks | Status: Planned. The list is not virtualized yet; the design keeps group counts exact when it is |

## Don't
- Don't use the note to restate the status. It carries the reason or the next step.
- Don't reorder rows under the pointer; apply the new order once the pointer leaves the list.
