import { invoke } from "@tauri-apps/api/core";
import { createSignal, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import { Icon } from "./Icon";

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

/** Bottom bar. Memory and CPU cover the app and every process it started, plus the terminal runtime. */
export function StatusBar(props: { sessions: number }) {
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
    <footer class="statusbar">
      <span>
        <i class="dot" />
        {t("statusbar.core")}
      </span>
      <span>
        <Icon name="terminal" class="icon-sm" />
        {t("statusbar.sessions")} <b>{props.sessions}</b>
      </span>
      <span class="statusbar-end">
        <Icon name="memory" class="icon-sm" />
        {t("statusbar.memory")} <b>{usage() ? formatMemory(usage()!.memoryBytes) : "–"}</b>
        <Icon name="cpu" class="icon-sm" />
        {t("statusbar.cpu")} <b>{usage() ? `${Math.round(usage()!.cpuPercent)}%` : "–"}</b>
      </span>
    </footer>
  );
}
