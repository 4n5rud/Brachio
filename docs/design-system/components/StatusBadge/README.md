# StatusBadge

A task's agent state shown as a glyph, a color and a word together, so it reads without color.

## The six states
| State | Glyph | Token | Label |
|---|---|---|---|
| working | arc turning on a faint ring | `color-status-working` | Working |
| blocked (permission, question, sign in) | filled diamond | `color-status-blocked` | Needs you |
| idle | hollow ring | `color-status-ready` | Ready for review |
| error, or exited with a code | filled square with an x | `color-status-error` | Error, or Crashed |
| exited 0 | filled check circle | `color-status-neutral` | Done |
| unknown | dashed ring | `color-status-neutral` | Unknown |

## Rules
- The working arc is the only repeating animation in Brachio (1.1s per turn, linear). It stops under `prefers-reduced-motion`.
- The glyph may stand alone only where the words sit in the same row (task rows, tiles, toasts). Headers and the inspector show glyph and label.
- Glyphs are 12px. The label uses `label` (13/500) in the state color.
- Why a task is blocked (permission, question, sign in) goes in the task's note, not in the badge.

## Don't
- Don't add a seventh color for a sub-state. Put the words in the note.
- Don't animate the diamond, the ring or the count badge to draw attention.
