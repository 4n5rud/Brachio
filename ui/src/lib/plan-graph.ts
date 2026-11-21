import type { ChildView } from "./orchestration";

export const NODE_WIDTH = 168;
export const NODE_HEIGHT = 64;
export const COLUMN_GAP = 40;
export const ROW_GAP = 12;

export interface PlanNode {
  id: string;
  layer: number;
  x: number;
  y: number;
}

export interface PlanEdge {
  from: string;
  to: string;
  /** The task it waits for is completed. */
  satisfied: boolean;
  /** Start and end points of the line, on the facing sides of the two cards. */
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface PlanLayout {
  nodes: PlanNode[];
  edges: PlanEdge[];
  width: number;
  height: number;
}

/** Layers are columns: a task sits one column right of the furthest task it waits for. */
export function planLayout(children: readonly ChildView[]): PlanLayout {
  const byId = new Map(children.map((child) => [child.id, child]));
  const layers = new Map<string, number>();
  const layerOf = (id: string, visiting: Set<string>): number => {
    const known = layers.get(id);
    if (known !== undefined) return known;
    const child = byId.get(id);
    if (!child || visiting.has(id)) return 0;
    visiting.add(id);
    const waits = child.dependencies.filter((dependency) => byId.has(dependency.id));
    const layer = waits.length === 0 ? 0 : 1 + Math.max(...waits.map((dependency) => layerOf(dependency.id, visiting)));
    visiting.delete(id);
    layers.set(id, layer);
    return layer;
  };
  const nextRow = new Map<number, number>();
  const nodes: PlanNode[] = children.map((child) => {
    const layer = layerOf(child.id, new Set());
    const row = nextRow.get(layer) ?? 0;
    nextRow.set(layer, row + 1);
    return {
      id: child.id,
      layer,
      x: layer * (NODE_WIDTH + COLUMN_GAP),
      y: row * (NODE_HEIGHT + ROW_GAP),
    };
  });
  const at = new Map(nodes.map((node) => [node.id, node]));
  const edges: PlanEdge[] = [];
  for (const child of children) {
    const to = at.get(child.id);
    for (const dependency of child.dependencies) {
      const from = at.get(dependency.id);
      if (!from || !to) continue;
      edges.push({
        from: from.id,
        to: to.id,
        satisfied: dependency.state === "completed",
        x1: from.x + NODE_WIDTH,
        y1: from.y + NODE_HEIGHT / 2,
        x2: to.x,
        y2: to.y + NODE_HEIGHT / 2,
      });
    }
  }
  const layerCount = nodes.reduce((most, node) => Math.max(most, node.layer + 1), 0);
  const rowCount = [...nextRow.values()].reduce((most, rows) => Math.max(most, rows), 0);
  return {
    nodes,
    edges,
    width: layerCount * NODE_WIDTH + Math.max(layerCount - 1, 0) * COLUMN_GAP,
    height: rowCount * NODE_HEIGHT + Math.max(rowCount - 1, 0) * ROW_GAP,
  };
}
