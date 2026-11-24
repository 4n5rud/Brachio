import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { t } from "../i18n";
import { settled } from "../lib/settled";
import { commitGraph, listCommits, readCommitDiff, type GraphRow } from "../lib/commit-history";
import { diffFiles } from "../lib/diff";
import { formatAge } from "../lib/time";
import { DiffReview } from "./DiffReview";
import { FileRow } from "./FileRow";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import { IconButton } from "./ui/IconButton";
import "./TaskCommits.css";

const LANE = 12;
const ROW = 28;
const LANE_COLORS = ["--color-accent-text", "--color-status-working", "--color-add", "--color-text-3"];
const laneColor = (lane: number) => `var(${LANE_COLORS[lane % LANE_COLORS.length]})`;
const laneX = (lane: number) => lane * LANE + LANE / 2;

/** One row of the branch graph: lanes passing through, the commit's dot, and merge links. */
function GraphCell(props: { row: GraphRow; width: number }) {
  const mid = ROW / 2;
  const row = () => props.row;
  return (
    <svg class="commit-graph" width={props.width * LANE} height={ROW} aria-hidden="true">
      <For each={row().before}>{(hash, lane) => (
        <Show when={hash !== ""}>
          <line x1={laneX(lane())} y1={0} x2={laneX(hash === row().before[row().column] && lane() !== row().column ? row().column : lane())}
            y2={mid} stroke={laneColor(lane())} stroke-width="1.5" />
        </Show>
      )}</For>
      <For each={row().after}>{(lane, index) => (
        <Show when={lane.hash !== ""}>
          <line x1={laneX(lane.from)} y1={mid} x2={laneX(index())} y2={ROW} stroke={laneColor(index())} stroke-width="1.5" />
        </Show>
      )}</For>
      <For each={row().joins}>{(lane) => (
        <line x1={laneX(row().column)} y1={mid} x2={laneX(lane)} y2={ROW} stroke={laneColor(lane)} stroke-width="1.5" />
      )}</For>
      <circle cx={laneX(row().column)} cy={mid} r="3.5" fill={laneColor(row().column)} />
    </svg>
  );
}

/** The task's commits with a simple branch graph, and the files and diff of the chosen one. Read only. */
export function TaskCommits(props: {
  taskId: string;
  now: number;
  split: boolean;
  onSplit: (split: boolean) => void;
  onDraft: (text: string) => Promise<boolean>;
}) {
  const [commits, { refetch }] = createResource(() => props.taskId, (id) => settled(listCommits(id)));
  const [selected, setSelected] = createSignal<string>();
  const [diff] = createResource(
    () => { const hash = selected(); return hash === undefined ? undefined : ([props.taskId, hash] as const); },
    ([id, hash]) => settled(readCommitDiff(id, hash)),
  );
  const graph = createMemo(() => commitGraph(commits()?.data ?? []));
  const width = createMemo(() => Math.max(1, ...graph().map((row) => Math.max(row.before.length, row.after.length, row.column + 1))));
  const files = createMemo(() => diffFiles(diff()?.data?.patch ?? ""));

  return (
    <InspectorSection title={t("commits.title")}
      end={<IconButton size="sm" icon="refresh" label={t("inspector.refresh")} busy={commits.loading} onClick={() => void refetch()} />}>
      <Show when={commits()?.error}>{(error) => <InspectorNote tone="error">{error()}</InspectorNote>}</Show>
      <Show when={!commits.loading && !commits()?.error && (commits()?.data ?? []).length === 0}>
        <InspectorNote>{t("commits.empty")}</InspectorNote>
      </Show>
      <For each={commits()?.data ?? []}>{(commit, index) => (
        <button type="button" class="commit-row" data-current={selected() === commit.hash || undefined}
          aria-pressed={selected() === commit.hash} title={`${commit.hash.slice(0, 7)} ${commit.author}`}
          onClick={() => setSelected(selected() === commit.hash ? undefined : commit.hash)}>
          <Show when={graph()[index()]}>{(row) => <GraphCell row={row()} width={width()} />}</Show>
          <span class="commit-subject">{commit.subject}</span>
          <For each={commit.refs}>{(name) => <span class="commit-ref">{name}</span>}</For>
          <span class="commit-age">{formatAge(commit.committedAtMs, props.now)}</span>
        </button>
      )}</For>
      <Show when={selected() !== undefined && diff()?.error}>{(error) => <InspectorNote tone="error">{error()}</InspectorNote>}</Show>
      <Show when={selected() !== undefined && diff()?.data}>
        {(data) => (
          <>
            <Show when={files().length === 0}><InspectorNote>{t("commits.noFiles")}</InspectorNote></Show>
            <For each={files()}>{(file) => (
              <FileRow mark={file.status} path={file.path} additions={file.additions} deletions={file.deletions} />
            )}</For>
            <Show when={data().patch}>
              <DiffReview patch={data().patch} split={props.split} onSplit={props.onSplit} onDraft={props.onDraft} />
            </Show>
            <Show when={data().truncated}><InspectorNote>{t("inspector.truncated")}</InspectorNote></Show>
          </>
        )}
      </Show>
    </InspectorSection>
  );
}
