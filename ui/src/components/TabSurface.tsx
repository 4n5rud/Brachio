import { Show, createResource, type Component } from "solid-js";
import { Dynamic } from "solid-js/web";
import { t } from "../i18n";
import { surfaceOf, type TabKind } from "../lib/tab-kinds";
import type { ContentSurface } from "../lib/tab-content";
import { saveTabState, tabState } from "../lib/tab-content";
import type { TaskTab } from "../lib/task-tabs";

/** What a file or browser view gets from the host. */
export interface TabSurfaceProps {
  taskId: string;
  /** The task's folder; a file tab's target is relative to it. */
  root: string;
  tab: TaskTab;
  /** The tab's stable key, for anything the view stores. */
  tabKey: string;
  /** What the view handed to `saveState` before it was last released, if anything. */
  state: unknown;
  /** Keeps the view's state (text, cursor, scroll, address) while its view is released. */
  saveState: (state: unknown) => void;
  /** Tells the host whether the tab holds edits that closing it would lose. */
  setDirty: (dirty: boolean) => void;
  theme?: string;
}

type SurfaceLoader = () => Promise<Component<TabSurfaceProps>>;

const loaders = new Map<ContentSurface, SurfaceLoader>();

/**
 * Names the view a surface draws. The loader runs the first time a tab of that surface is shown,
 * so a view and its dependencies can stay in their own chunk behind a dynamic `import()`.
 */
export function registerTabSurface(surface: ContentSurface, load: SurfaceLoader): () => void {
  loaders.set(surface, load);
  return () => {
    if (loaders.get(surface) === load) loaders.delete(surface);
  };
}

const loaded = new Map<SurfaceLoader, Promise<Component<TabSurfaceProps>>>();
const loadOnce = (load: SurfaceLoader) => {
  let pending = loaded.get(load);
  if (!pending) {
    pending = load().catch((failure: unknown) => {
      loaded.delete(load);
      throw failure;
    });
    loaded.set(load, pending);
  }
  return pending;
};

/**
 * The one place a file or browser tab's view is mounted. The view exists only while the tab is
 * shown: a hidden tab releases it and keeps what the view saved, so many open tabs cost little.
 */
export function TabContentHost(props: {
  taskId: string;
  root: string;
  tab: TaskTab;
  tabKey: string;
  name?: string;
  shown: boolean;
  onDirty: (dirty: boolean) => void;
  theme?: string;
}) {
  const surface = () => surfaceOf(props.tab.kind) as ContentSurface;
  return (
    <Show when={props.shown}>
      <Show when={loaders.get(surface())} keyed
        fallback={<TabSurfacePlaceholder kind={props.tab.kind} name={props.name} />}>
        {(load) => <LoadedSurface load={load} {...props} />}
      </Show>
    </Show>
  );
}

function LoadedSurface(props: {
  load: SurfaceLoader;
  taskId: string;
  root: string;
  tab: TaskTab;
  tabKey: string;
  name?: string;
  onDirty: (dirty: boolean) => void;
  theme?: string;
}) {
  const [view] = createResource(() => props.load, loadOnce);
  // The state is read once per mount: a view saves into it and reads it back on its next mount.
  // Unsaved edits outlive the view, so releasing it leaves the tab's dirty mark as it is.
  const state = tabState(props.tabKey);
  return (
    <Show when={!view.error} fallback={<TabSurfaceFailed name={props.name} error={String(view.error)} />}>
      <Show when={view()} fallback={<div class="tab-surface-placeholder" role="status" aria-busy="true" />}>
        {(component) => (
          <Dynamic component={component()} taskId={props.taskId} root={props.root} tab={props.tab}
            tabKey={props.tabKey} state={state} theme={props.theme}
            saveState={(next: unknown) => saveTabState(props.tabKey, next)}
            setDirty={props.onDirty} />
        )}
      </Show>
    </Show>
  );
}

function TabSurfaceFailed(props: { name?: string; error: string }) {
  return (
    <div class="tab-surface-placeholder" role="alert">
      <strong>{props.name}</strong>
      <span>{t("tab.loadFailed")}</span>
      <code>{props.error}</code>
    </div>
  );
}

/** Fills the tab area of a tab whose view this build does not have yet. */
export function TabSurfacePlaceholder(props: { kind: TabKind; name?: string }) {
  return (
    <div class="tab-surface-placeholder" data-surface={surfaceOf(props.kind)} role="status">
      <strong>{props.name}</strong>
      <span>{t("tab.notYet")}</span>
    </div>
  );
}
