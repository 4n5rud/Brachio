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
  sessions: number;
  coreStopped?: boolean;
  memory?: string;
  /** Tooltip that splits the memory figure into its parts. */
  memoryTitle?: string;
  cpu?: string;
  /** Current branch of the repository and its distance from the remote. */
  branch?: string;
  ahead?: number;
  behind?: number;
}) {
  return (
    <footer class="statusbar">
      <span>
        <Show when={props.coreStopped} fallback={<i class="dot" />}>
          <StatusGlyph tone="error" />
        </Show>
        {t(props.coreStopped ? "statusbar.coreStopped" : "statusbar.coreOk")}
      </span>
      <span>
        {t("statusbar.sessions")} <b>{props.sessions}</b>
      </span>
      <span title={props.memoryTitle}>
        <Icon name="memory" class="icon-xs" />
        {t("statusbar.memory")} <b>{props.memory ?? "–"}</b>
      </span>
      <span>
        <Icon name="cpu" class="icon-xs" />
        {t("statusbar.cpu")} <b>{props.cpu ?? "–"}</b>
      </span>
      <Show when={props.branch}>
        {(branch) => (
          <span class="statusbar-end">
            <Icon name="branch" class="icon-xs" />
            {branch()} <b>↑{props.ahead ?? 0} ↓{props.behind ?? 0}</b>
          </span>
        )}
      </Show>
    </footer>
  );
}

/** Bottom bar. Memory and CPU cover the app and every process it started, plus the terminal runtime. */
export function StatusBar(props: { sessions: number }) {
  const [usage, setUsage] = createSignal<ResourceUsage | null>(null);
  const [coreStopped, setCoreStopped] = createSignal(false);

  onMount(() => {
    if (!inTauri) return;
    const refresh = () =>
      void invoke<ResourceUsage>("resource_usage").then(
        (next) => { setUsage(next); setCoreStopped(false); },
        () => setCoreStopped(true),
      );
    refresh();
    const timer = window.setInterval(refresh, REFRESH_MS);
    onCleanup(() => window.clearInterval(timer));
  });
  const memoryTitle = () => {
    const current = usage();
    if (!current) return undefined;
    return t("statusbar.memoryParts")
      .replace("{app}", formatMemory(current.memoryBytes - current.runtimeMemoryBytes))
      .replace("{runtime}", formatMemory(current.runtimeMemoryBytes));
  };

  return (
    <StatusBarView
      sessions={props.sessions}
      coreStopped={coreStopped()}
      memory={usage() ? formatMemory(usage()!.memoryBytes) : undefined}
      memoryTitle={memoryTitle()}
      cpu={usage() ? `${Math.round(usage()!.cpuPercent)}%` : undefined}
    />
  );
}
