import { invoke } from "@tauri-apps/api/core";
import { Show, createSignal, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import { Icon } from "./Icon";
import { StatusGlyph } from "./ui/StatusBadge";
import "./StatusBar.css";

const REFRESH_MS = 3000;
const inTauri = "__TAURI_INTERNALS__" in window;

interface ResourceUsage {
  memoryBytes: number;
  runtimeMemoryBytes: number;
  cpuPercent: number;
}

export function formatMemory(bytes: number): string {
  const megabytes = bytes / (1024 * 1024);
  return megabytes >= 1024 ? `${(megabytes / 1024).toFixed(1)} GB` : `${Math.round(megabytes)} MB`;
}

/** What the bar shows. A missing number prints as a dash. */
export function StatusBarView(props: {
  agents: number;
  /** Agents waiting on the person; the dot becomes the Needs you diamond with this count. */
  needsYou?: number;
  memory?: string;
  cpu?: string;
  /** Current branch of the repository and its distance from the remote. */
  branch?: string;
  ahead?: number;
  behind?: number;
}) {
  return (
    <footer class="statusbar">
      <span>
        <Show when={props.needsYou} fallback={<i class="dot" />}>
          {(count) => (
            <span class="statusbar-needs" title={t("group.needs")} aria-label={`${t("group.needs")} ${count()}`}>
              <StatusGlyph tone="blocked" />
              <b>{count()}</b>
            </span>
          )}
        </Show>
        {t(props.agents === 1 ? "statusbar.agentOne" : "statusbar.agentMany").replace("{count}", String(props.agents))}
      </span>
      <span>
        <Icon name="memory" class="icon-sm" />
        {t("statusbar.memory")} <b>{props.memory ?? "–"}</b>
      </span>
      <span>
        <Icon name="cpu" class="icon-sm" />
        {t("statusbar.cpu")} <b>{props.cpu ?? "–"}</b>
      </span>
      <Show when={props.branch}>
        {(branch) => (
          <span class="statusbar-end">
            <Icon name="branch" class="icon-sm" />
            {branch()} <b>↑{props.ahead ?? 0} ↓{props.behind ?? 0}</b>
          </span>
        )}
      </Show>
    </footer>
  );
}

/** Bottom bar. Memory and CPU cover the app and every process it started, plus the terminal runtime. */
export function StatusBar(props: { sessions: number; needsYou?: number }) {
  const [usage, setUsage] = createSignal<ResourceUsage | null>(null);

  onMount(() => {
    if (!inTauri) return;
    const refresh = () =>
      void invoke<ResourceUsage>("resource_usage").then(setUsage, () => setUsage(null));
    refresh();
    const timer = window.setInterval(refresh, REFRESH_MS);
    onCleanup(() => window.clearInterval(timer));
  });

  return (
    <StatusBarView
      agents={props.sessions}
      needsYou={props.needsYou}
      memory={usage() ? formatMemory(usage()!.memoryBytes) : undefined}
      cpu={usage() ? `${Math.round(usage()!.cpuPercent)}%` : undefined}
    />
  );
}
