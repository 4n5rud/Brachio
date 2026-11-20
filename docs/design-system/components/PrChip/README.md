# PrChip

The pull request of a task in one small chip: number in mono, then the one fact that decides what to do next.

## Order of precedence
conflicts, CI failed, changes requested, CI running, CI passed, open, then merged or closed. Only the first that applies shows.

## Look
18px high, `radius-xs`, 1px `color-line`. Number in `meta` `color-text-3`; the word in the status color it stands for: failed and conflicts `color-status-error`, changes requested `color-status-blocked`, running `color-status-working`, passed `color-status-ready`, others `color-text-3`. When GitHub cannot be reached the chip keeps the last result, turns its edge dashed and adds "offline"; the tooltip says why.

## Placement
In the TaskRow it sits on its own line under the meta line, with the unread mail count, so the branch and diff stat never get squeezed. (The app joins them with a middle dot today; the design replaces the dot with layout.)
