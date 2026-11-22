import { invoke } from "@tauri-apps/api/core";
import { Show, createSignal, onCleanup, onMount, type JSX } from "solid-js";
import { t } from "../i18n";
import { agentUsageRefreshing, agentUsageUpdatedAt, agentUsages, refreshAgentUsage, startAgentUsagePolling } from "../lib/agent-usage";
import { UsageStatus } from "./AgentUsage";
import { Icon } from "./Icon";
import { Button } from "./ui/Button";
import { Popover } from "./ui/Popover";
import { StatusGlyph } from "./ui/StatusBadge";
import "./StatusBar.css";

const REFRESH_MS = 3000;
const CLOCK_MS = 30_000;
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

/** One summary in the bar; sessions, memory and CPU open in a popover. */
function ResourceSummary(props: {
  sessions: number;
  usageUnavailable?: boolean;
  memory?: string;
  memoryTitle?: string;
  cpu?: string;
}) {
  const [anchor, setAnchor] = createSignal<HTMLElement>();
  return (
    <>
      <button type="button" class="statusbar-summary" aria-haspopup="dialog" aria-expanded={anchor() !== undefined}
        onClick={(event) => setAnchor(anchor() ? undefined : event.currentTarget)}>
        <Show when={props.usageUnavailable} fallback={<i class="dot" />}>
          <StatusGlyph tone="error" />
        </Show>
        {t(props.usageUnavailable ? "statusbar.usageUnavailable" : "statusbar.ok")}
        <span>{t("statusbar.sessions")} <b>{props.sessions}</b></span>
      </button>
      <Show when={anchor()}>
        {(element) => (
          <Popover anchor={element()} label={t("statusbar.details")} onClose={() => setAnchor(undefined)}>
            <dl class="statusbar-details">
              <dt>{t("statusbar.sessions")}</dt><dd>{props.sessions}</dd>
              <dt><Icon name="memory" class="icon-xs" />{t("statusbar.memory")}</dt><dd>{props.memory ?? "–"}</dd>
              <Show when={props.memoryTitle}>{(parts) => <dd class="statusbar-details-note">{parts()}</dd>}</Show>
              <dt><Icon name="cpu" class="icon-xs" />{t("statusbar.cpu")}</dt><dd>{props.cpu ?? "–"}</dd>
            </dl>
            <Show when={props.usageUnavailable}>
              <p class="statusbar-details-error">{t("statusbar.usageUnavailableNote")}</p>
              <Button size="sm" variant="ghost" onClick={() => window.location.reload()}>{t("statusbar.reload")}</Button>
            </Show>
          </Popover>
        )}
      </Show>
    </>
  );
}

/** What the bar shows. A missing number prints as a dash. */
export function StatusBarView(props: {
  sessions: number;
  /** The app could not read its own memory and CPU use. */
  usageUnavailable?: boolean;
  memory?: string;
  /** Tooltip that splits the memory figure into its parts. */
  memoryTitle?: string;
  cpu?: string;
  /** Current branch of the repository and its distance from the remote. */
  branch?: string;
  ahead?: number;
  behind?: number;
  /** Agent usage readout, at the left edge. */
  usage?: JSX.Element;
}) {
  return (
    <footer class="statusbar">
      {props.usage}
      <ResourceSummary sessions={props.sessions} usageUnavailable={props.usageUnavailable} memory={props.memory}
        memoryTitle={props.memoryTitle} cpu={props.cpu} />
      <Show when={props.branch}>
        {(branch) => (
          <span class="statusbar-end">
            <Icon name="ghBranch" class="icon-xs" />
            {branch()} <b>↑{props.ahead ?? 0} ↓{props.behind ?? 0}</b>
          </span>
        )}
      </Show>
    </footer>
  );
}

/** Bottom bar. Memory and CPU cover the app and every process it started, plus the terminal runtime. */
export function StatusBar(props: { sessions: number; onAgentSettings: () => void }) {
  const [usage, setUsage] = createSignal<ResourceUsage | null>(null);
  const [usageUnavailable, setUsageUnavailable] = createSignal(false);
  const [now, setNow] = createSignal(Date.now() / 1000);

  onMount(() => {
    if (!inTauri) return;
    const refresh = () =>
      void invoke<ResourceUsage>("resource_usage").then(
        (next) => { setUsage(next); setUsageUnavailable(false); },
        () => setUsageUnavailable(true),
      );
    refresh();
    const timer = window.setInterval(refresh, REFRESH_MS);
    const clock = window.setInterval(() => setNow(Date.now() / 1000), CLOCK_MS);
    const stopUsage = startAgentUsagePolling();
    onCleanup(() => { window.clearInterval(timer); window.clearInterval(clock); stopUsage(); });
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
      usageUnavailable={usageUnavailable()}
      memory={usage() ? formatMemory(usage()!.memoryBytes) : undefined}
      memoryTitle={memoryTitle()}
      cpu={usage() ? `${Math.round(usage()!.cpuPercent)}%` : undefined}
      usage={
        <UsageStatus usages={agentUsages()} now={now()} refreshing={agentUsageRefreshing()} updatedAt={agentUsageUpdatedAt()}
          onRefresh={() => void refreshAgentUsage(true)} onAccounts={props.onAgentSettings}
          onDetails={agentUsages().some((entry) => entry.agent === "claude") ? () => void invoke("open_claude_usage") : undefined} />
      }
    />
  );
}
