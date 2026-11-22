import { For, Show, createSignal, type JSX } from "solid-js";
import { t } from "../../i18n";
import "./Chart.css";

export interface ChartSeries {
  label: string;
  /** A CSS color value, normally a design token such as `var(--color-accent)`. */
  color: string;
}

const WIDTH = 600;
const HEIGHT = 150;
const LEFT = 38;
const BOTTOM = 18;
const TOP = 6;
const RIGHT = 4;

function Legend(props: { series: readonly ChartSeries[] }) {
  return (
    <ul class="chart-legend">
      <For each={props.series}>{(item) => (
        <li><i style={{ background: item.color }} />{item.label}</li>
      )}</For>
    </ul>
  );
}

/** The data as a table, for people who cannot see or point at the picture. */
function TableFallback(props: { caption: string; head: readonly string[]; rows: readonly (readonly string[])[] }) {
  return (
    <details class="chart-table">
      <summary>{t("chart.table")}</summary>
      <table>
        <caption>{props.caption}</caption>
        <thead><tr><For each={props.head}>{(cell) => <th scope="col">{cell}</th>}</For></tr></thead>
        <tbody><For each={props.rows}>{(row) => (
          <tr><For each={row}>{(cell, at) => (at() === 0 ? <th scope="row">{cell}</th> : <td>{cell}</td>)}</For></tr>
        )}</For></tbody>
      </table>
    </details>
  );
}

/**
 * Bars per category. With several series they are stacked. The label names the chart; the numbers
 * are also in a table below, and hovering a bar shows its values.
 */
export function BarChart(props: {
  label: string;
  categories: readonly string[];
  series: readonly (ChartSeries & { values: readonly number[] })[];
  format: (value: number) => string;
  /** How a category is written on the axis, for example `11-03`. */
  axis?: (category: string) => string;
}) {
  const [hover, setHover] = createSignal<number>();
  const totals = () => props.categories.map((_, at) => props.series.reduce((sum, item) => sum + (item.values[at] ?? 0), 0));
  const top = () => Math.max(1, ...totals());
  const plotWidth = WIDTH - LEFT - RIGHT;
  const plotHeight = HEIGHT - BOTTOM - TOP;
  const slot = () => plotWidth / Math.max(1, props.categories.length);
  const y = (value: number) => TOP + plotHeight - (value / top()) * plotHeight;
  const ticks = () => {
    const every = Math.max(1, Math.ceil(props.categories.length / 6));
    return props.categories.map((category, at) => ({ category, at })).filter((tick) => tick.at % every === 0 || tick.at === props.categories.length - 1);
  };
  return (
    <figure class="chart">
      <div class="chart-plot">
        <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} role="img" aria-label={props.label} preserveAspectRatio="xMidYMid meet"
          onPointerLeave={() => setHover(undefined)}>
          <line class="chart-axis" x1={LEFT} x2={WIDTH - RIGHT} y1={y(0)} y2={y(0)} />
          <line class="chart-grid" x1={LEFT} x2={WIDTH - RIGHT} y1={y(top())} y2={y(top())} />
          <text class="chart-tick" x={LEFT - 4} y={y(top()) + 3} text-anchor="end">{props.format(top())}</text>
          <text class="chart-tick" x={LEFT - 4} y={y(0)} text-anchor="end">0</text>
          <For each={props.categories}>{(_, at) => {
            let stacked = 0;
            return (
              <g onPointerEnter={() => setHover(at())}>
                <rect class="chart-hit" x={LEFT + at() * slot()} y={TOP} width={slot()} height={plotHeight} />
                <For each={props.series}>{(item) => {
                  const value = item.values[at()] ?? 0;
                  const base = stacked;
                  stacked += value;
                  return value > 0 ? (
                    <rect x={LEFT + at() * slot() + slot() * 0.15} width={slot() * 0.7} rx="1.5"
                      y={y(base + value)} height={Math.max(0.5, y(base) - y(base + value))} fill={item.color} />
                  ) : null;
                }}</For>
              </g>
            );
          }}</For>
          <For each={ticks()}>{(tick) => (
            <text class="chart-tick" x={LEFT + tick.at * slot() + slot() / 2} y={HEIGHT - 4} text-anchor="middle">
              {props.axis ? props.axis(tick.category) : tick.category}
            </text>
          )}</For>
        </svg>
        <Show when={hover()}>{(at) => (
          <div class="chart-tip" role="status" style={{ left: `${Math.min(80, Math.max(0, ((LEFT + at() * slot()) / WIDTH) * 100))}%` }}>
            <strong>{props.categories[at()]}</strong>
            <For each={props.series}>{(item) => (
              <span><i style={{ background: item.color }} />{item.label} {props.format(item.values[at()] ?? 0)}</span>
            )}</For>
          </div>
        )}</Show>
      </div>
      <Show when={props.series.length > 1}><Legend series={props.series} /></Show>
      <TableFallback caption={props.label} head={["", ...props.series.map((item) => item.label)]}
        rows={props.categories.map((category, at) => [category, ...props.series.map((item) => props.format(item.values[at] ?? 0))])} />
    </figure>
  );
}

export interface LinePoint {
  at: number;
  value: number;
}

/** Lines of a 0 to 100 value over time, such as the share of a plan limit used. */
export function LineChart(props: {
  label: string;
  from: number;
  to: number;
  series: readonly (ChartSeries & { points: readonly LinePoint[] })[];
  /** How a moment is written in the tooltip and table. */
  when: (at: number) => string;
}) {
  const plotWidth = WIDTH - LEFT - RIGHT;
  const plotHeight = HEIGHT - BOTTOM - TOP;
  const x = (at: number) => LEFT + ((at - props.from) / Math.max(1, props.to - props.from)) * plotWidth;
  const y = (value: number) => TOP + plotHeight - (Math.min(100, Math.max(0, value)) / 100) * plotHeight;
  return (
    <figure class="chart">
      <div class="chart-plot">
        <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} role="img" aria-label={props.label} preserveAspectRatio="xMidYMid meet">
          <line class="chart-axis" x1={LEFT} x2={WIDTH - RIGHT} y1={y(0)} y2={y(0)} />
          <line class="chart-grid" x1={LEFT} x2={WIDTH - RIGHT} y1={y(100)} y2={y(100)} />
          <text class="chart-tick" x={LEFT - 4} y={y(100) + 3} text-anchor="end">100%</text>
          <text class="chart-tick" x={LEFT - 4} y={y(0)} text-anchor="end">0%</text>
          <For each={props.series}>{(item) => (
            <>
              <polyline fill="none" stroke={item.color} stroke-width="1.75" stroke-linejoin="round"
                points={item.points.map((point) => `${x(point.at)},${y(point.value)}`).join(" ")} />
              <For each={item.points}>{(point) => (
                <circle cx={x(point.at)} cy={y(point.value)} r="2.5" fill={item.color}>
                  <title>{`${item.label}, ${props.when(point.at)}: ${Math.round(point.value)}%`}</title>
                </circle>
              )}</For>
            </>
          )}</For>
        </svg>
      </div>
      <Legend series={props.series} />
      <TableFallback caption={props.label} head={[t("chart.time"), t("chart.series"), t("chart.percent")]}
        rows={props.series.flatMap((item) => item.points.map((point): string[] => [props.when(point.at), item.label, `${Math.round(point.value)}%`]))} />
    </figure>
  );
}

/** Horizontal bars of shares of one total, each with its value written beside it. */
export function ShareList(props: {
  label: string;
  rows: readonly { key: string; name: string; value: number; icon?: JSX.Element }[];
  format: (value: number) => string;
  empty: string;
}) {
  const top = () => Math.max(1, ...props.rows.map((row) => row.value));
  return (
    <Show when={props.rows.length > 0} fallback={<p class="form-hint">{props.empty}</p>}>
      <ul class="share-list" aria-label={props.label}>
        <For each={props.rows}>{(row) => (
          <li>
            <span class="share-name" title={row.name}>{row.icon}{row.name}</span>
            <span class="share-bar"><i style={{ width: `${(row.value / top()) * 100}%` }} /></span>
            <span class="share-value">{props.format(row.value)}</span>
          </li>
        )}</For>
      </ul>
    </Show>
  );
}
