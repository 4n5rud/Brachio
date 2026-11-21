import { For, createMemo } from "solid-js";
import { t } from "../i18n";
import type { ChildView } from "../lib/orchestration";
import { NODE_HEIGHT, NODE_WIDTH, planLayout } from "../lib/plan-graph";
import { StateChip, stateText } from "./OrchestrationState";
import "./PlanGraph.css";

const curve = (x1: number, y1: number, x2: number, y2: number) => {
  const bend = (x2 - x1) / 2;
  return `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`;
};

/**
 * Child tasks in columns, with a line from each task to the tasks that wait for it. A solid green line is satisfied;
 * a dashed one still waits. Arrow keys move between cards: right to a task waiting on this one, left to one it waits for.
 */
export function PlanGraph(props: {
  children: readonly ChildView[];
  selectedId?: string;
  onSelect: (id: string) => void;
}) {
  const layout = createMemo(() => planLayout(props.children));
  const byId = () => new Map(props.children.map((child) => [child.id, child]));
  let root: HTMLDivElement | undefined;
  const focusNode = (id: string | undefined) => {
    if (!id) return;
    root?.querySelector<HTMLElement>(`[data-node-id="${CSS.escape(id)}"]`)?.focus();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    const id = (event.target instanceof Element ? event.target.closest<HTMLElement>("[data-node-id]") : null)?.dataset.nodeId;
    if (!id || event.altKey || event.ctrlKey || event.metaKey) return;
    const current = layout().nodes.find((node) => node.id === id);
    if (!current) return;
    const sameLayer = layout().nodes.filter((node) => node.layer === current.layer);
    const index = sameLayer.findIndex((node) => node.id === id);
    let next: string | undefined;
    if (event.key === "ArrowRight") next = layout().edges.find((edge) => edge.from === id)?.to;
    else if (event.key === "ArrowLeft") next = layout().edges.find((edge) => edge.to === id)?.from;
    else if (event.key === "ArrowDown") next = sameLayer[index + 1]?.id;
    else if (event.key === "ArrowUp") next = sameLayer[index - 1]?.id;
    else return;
    event.preventDefault();
    focusNode(next);
  };
  const label = (child: ChildView) => {
    const waits = child.dependencies.map((dependency) => dependency.title).join(", ");
    return waits
      ? `${child.title}, ${stateText(child.state)}, ${t("orchestration.dependsOn")} ${waits}`
      : `${child.title}, ${stateText(child.state)}`;
  };
  return (
    <div class="plan-scroll">
      <div ref={root} class="plan-graph" role="group" aria-label={t("orchestration.graph")}
        style={{ width: `${layout().width}px`, height: `${layout().height}px` }} onKeyDown={onKeyDown}>
        <svg class="plan-lines" width={layout().width} height={layout().height} aria-hidden="true">
          <For each={layout().edges}>
            {(edge) => (
              <path d={curve(edge.x1, edge.y1, edge.x2, edge.y2)} class="plan-line"
                classList={{ "plan-line-done": edge.satisfied }} />
            )}
          </For>
        </svg>
        <For each={layout().nodes}>
          {(node) => {
            const child = () => byId().get(node.id)!;
            return (
              <button type="button" class="plan-node" data-node-id={node.id}
                classList={{ "plan-node-wait": ["queued", "blocked"].includes(child().state) }}
                aria-pressed={props.selectedId === node.id} aria-label={label(child())}
                style={{ left: `${node.x}px`, top: `${node.y}px`, width: `${NODE_WIDTH}px`, height: `${NODE_HEIGHT}px` }}
                onClick={() => props.onSelect(node.id)}>
                <b class="plan-node-title">{child().title}</b>
                <StateChip state={child().state} />
              </button>
            );
          }}
        </For>
      </div>
    </div>
  );
}
