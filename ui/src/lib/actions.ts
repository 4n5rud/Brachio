import { createSignal } from "solid-js";
import type { StatusTone } from "./status";

/** One thing the person can run from the command palette. */
export interface AppAction {
  /** Stable and unique, such as "app.settings". Registering the same id again replaces the action. */
  id: string;
  /** Read when shown, so it follows the language. */
  label: () => string;
  /** Section heading in the palette, such as "App" or "Task". */
  group: () => string;
  /** Shortcut as it is drawn on key caps; omit when there is none. */
  keys?: string;
  /** Hides the action while it cannot run, such as "Archive" with no task open. */
  available?: () => boolean;
  run: () => void | Promise<void>;
}

/** A row of Go To mode: an agent or terminal tab somewhere in the app. */
export interface GoToTarget {
  id: string;
  projectId: string;
  projectName: string;
  kind: "agent" | "terminal";
  title: string;
  path?: string;
  /** Quiet extra words, such as the tokens the session used. */
  detail?: string;
  tone: StatusTone;
  /** Focuses the task and the tab. */
  focus: () => void;
}

const [actionList, setActionList] = createSignal<readonly AppAction[]>([]);
const [goToSources, setGoToSources] = createSignal<readonly { key: symbol; read: () => readonly GoToTarget[] }[]>([]);

/** Adds actions to the palette; returns the function that removes them. Call it from a component or onCleanup. */
export function registerActions(...actions: AppAction[]): () => void {
  const ids = new Set(actions.map((action) => action.id));
  setActionList((list) => [...list.filter((item) => !ids.has(item.id)), ...actions]);
  return () => setActionList((list) => list.filter((item) => !actions.includes(item)));
}

/** The actions that can run now, in registration order. */
export const availableActions = (): AppAction[] => actionList().filter((action) => action.available?.() ?? true);

/** Adds a source of Go To rows; `read` is reactive, so rows follow the owner's state. Returns the remover. */
export function registerGoToSource(read: () => readonly GoToTarget[]): () => void {
  const key = Symbol("go-to");
  setGoToSources((list) => [...list, { key, read }]);
  return () => setGoToSources((list) => list.filter((item) => item.key !== key));
}

export const goToTargets = (): GoToTarget[] => goToSources().flatMap((source) => [...source.read()]);

/** Go To rows grouped by project, in first-seen order. */
export function groupByProject(targets: readonly GoToTarget[]): { projectId: string; projectName: string; targets: GoToTarget[] }[] {
  const groups = new Map<string, { projectId: string; projectName: string; targets: GoToTarget[] }>();
  for (const target of targets) {
    const group = groups.get(target.projectId) ?? { projectId: target.projectId, projectName: target.projectName, targets: [] };
    group.projectName ||= target.projectName;
    group.targets.push(target);
    groups.set(target.projectId, group);
  }
  return [...groups.values()];
}
