# Chart

Dependency-free SVG charts for the analytics view: `BarChart` (stacked or single series), `LineChart` (0 to 100 values over time) and `ShareList` (horizontal bars of one total).

## Use
- Colors come from tokens only (`color-accent`, `color-status-working`, `color-status-neutral`), so every theme and both light and dark work.
- Hovering a bar shows its values; every chart also has a "View as table" fallback with the same numbers, and the svg has an `aria-label`.
- The plot scales to its container (viewBox, `min-width: 0`), so it never causes horizontal scroll in the 720px settings column.

## Don't
- Don't encode meaning in color alone: the legend and the table name each series.
- Don't show money; the app shows tokens only.
