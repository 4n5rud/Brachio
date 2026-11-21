import { For, Match, Show, Switch, createSignal } from "solid-js";
import { t } from "../i18n";
import { AGENT_NAME } from "../lib/tasks";
import { formatRemaining, nearlyUsed, type AgentUsage, type RemainingUnit, type UsageWindow } from "../lib/agent-usage";
import { AgentIcon } from "./AgentIcon";
import { Icon } from "./Icon";
import { IconButton } from "./ui/IconButton";
import { Popover } from "./ui/Popover";
import { SegmentedControl } from "./ui/SegmentedControl";
import { StatusGlyph } from "./ui/StatusBadge";
import "./ui/Menu.css";
import "./AgentUsage.css";

export type UsageMode = "detailed" | "compact";

const unit = (kind: RemainingUnit, count: number) =>
  t(kind === "d" ? "usage.unitDay" : kind === "h" ? "usage.unitHour" : "usage.unitMinute").replace("{n}", String(count));

const percentText = (window: UsageWindow | null) => (window ? `${Math.round(window.usedPercent)}%` : "–");
const remainingText = (window: UsageWindow | null | undefined, now: number): string | null =>
  window?.resetsAt == null ? null : formatRemaining(window.resetsAt - now, unit);

/** One window's share used. The fill is the only encoding; the number beside it is text. */
export function UsageMeter(props: { window: UsageWindow | null; label: string }) {
  return (
    <span class="usage-meter" role="progressbar" aria-label={props.label} aria-valuemin={0} aria-valuemax={100}
      aria-valuenow={props.window ? Math.round(props.window.usedPercent) : undefined}>
      <i style={{ width: `${props.window?.usedPercent ?? 0}%` }} />
    </span>
  );
}

/** Status bar readout: per agent the 5-hour bar, percent and time to reset, then the weekly pair. */
export function UsageReadout(props: {
  usages: readonly AgentUsage[];
  /** Unix seconds. */
  now: number;
  refreshing?: boolean;
  onRefresh: () => void;
  open?: boolean;
  onToggle?: (anchor: HTMLElement) => void;
}) {
  return (
    <Show when={props.usages.length > 0}>
      <span class="usage-readout">
        <button type="button" class="usage-trigger" aria-haspopup="dialog" aria-expanded={props.open ?? false}
          onClick={(event) => props.onToggle?.(event.currentTarget)}>
          <span>{t("usage.label")}</span>
          <For each={props.usages}>{(usage) => (
            <span class="usage-agent">
              <AgentIcon agent={usage.agent} />
              <Switch>
                <Match when={usage.state === "ok" && usage}>{(ok) => (
                  <>
                    <UsageMeter window={ok().session} label={`${AGENT_NAME[usage.agent]} ${t("usage.fiveHour")}`} />
                    <b>{percentText(ok().session)}</b>
                    <small>{remainingText(ok().session, props.now)}</small>
                    <span aria-hidden="true">·</span>
                    <b>{percentText(ok().weekly)}</b>
                    <small>{remainingText(ok().weekly, props.now)}</small>
                  </>
                )}</Match>
                <Match when={usage.state === "unavailable"}>
                  <StatusGlyph tone="unknown" />{t("usage.unavailable")}
                </Match>
              </Switch>
            </span>
          )}</For>
        </button>
        <IconButton icon="refresh" size="sm" label={t("usage.refresh")} disabled={props.refreshing}
          class="usage-refresh" onClick={() => props.onRefresh()} />
      </span>
    </Show>
  );
}

function WindowRow(props: { label: string; window: UsageWindow | null; agent: string }) {
  return (
    <span class="usage-window">
      <span>{props.label}</span>
      <UsageMeter window={props.window} label={`${props.agent} ${props.label}`} />
      <b>{percentText(props.window)}</b>
      <Show when={props.window && nearlyUsed(props.window)}>
        <span class="usage-warning"><StatusGlyph tone="blocked" />{t("usage.nearlyUsed")}</span>
      </Show>
    </span>
  );
}

/** Popover body: one row per agent, then the two places to go next. */
export function UsagePanel(props: {
  usages: readonly AgentUsage[];
  now: number;
  mode: UsageMode;
  onMode: (mode: UsageMode) => void;
  refreshing?: boolean;
  onRefresh: () => void;
  onAccounts: () => void;
  /** Opens the provider usage page; offered while Claude Code is signed in. */
  onDetails?: () => void;
}) {
  return (
    <div class="usage-panel">
      <div class="usage-panel-head">
        <h3>{t("usage.title")}</h3>
        <IconButton icon="refresh" size="sm" label={t("usage.refresh")} disabled={props.refreshing}
          onClick={() => props.onRefresh()} />
        <SegmentedControl label={t("usage.view")} value={props.mode} onChange={props.onMode}
          options={[{ value: "detailed", label: t("usage.detailed") }, { value: "compact", label: t("usage.compact") }]} />
      </div>
      <For each={props.usages}>{(usage) => (
        <button type="button" class="usage-agent-row" onClick={() => props.onAccounts()}
          aria-label={t("usage.agentAccount").replace("{agent}", AGENT_NAME[usage.agent])}>
          <span class="usage-agent-name">
            <AgentIcon agent={usage.agent} />{AGENT_NAME[usage.agent]}
            <Show when={usage.state === "ok" && usage.plan}>{(plan) => <span class="usage-plan">{plan()}</span>}</Show>
            <Icon name="chevronRight" class="icon-sm usage-chevron" />
          </span>
          <Switch>
            <Match when={usage.state === "ok" && usage}>{(ok) => (
              <Show when={props.mode === "detailed"} fallback={
                <span class="usage-compact">
                  {t("usage.fiveHour")} <b>{percentText(ok().session)}</b> · {t("usage.week")} <b>{percentText(ok().weekly)}</b>
                </span>
              }>
                <Show when={remainingText(ok().session ?? ok().weekly, props.now)}>
                  {(time) => <span class="usage-reset">{t("usage.resetsIn").replace("{time}", time())}</span>}
                </Show>
                <WindowRow label={t("usage.fiveHour")} window={ok().session} agent={AGENT_NAME[usage.agent]} />
                <WindowRow label={t("usage.week")} window={ok().weekly} agent={AGENT_NAME[usage.agent]} />
              </Show>
            )}</Match>
            <Match when={usage.state === "unavailable"}>
              <span class="usage-reset"><StatusGlyph tone="unknown" />{t("usage.unavailable")}. {t("usage.unavailableNote")}</span>
            </Match>
          </Switch>
        </button>
      )}</For>
      <div class="usage-panel-foot">
        <Show when={props.onDetails}>
          <button type="button" class="menu-item" onClick={() => props.onDetails?.()}>
            {t("usage.details")}<Icon name="external" class="icon-sm usage-chevron" />
          </button>
        </Show>
        <button type="button" class="menu-item" onClick={() => props.onAccounts()}>{t("usage.manage")}</button>
      </div>
    </div>
  );
}

/** Readout with its popover, for the real status bar. */
export function UsageStatus(props: {
  usages: readonly AgentUsage[];
  now: number;
  refreshing?: boolean;
  onRefresh: () => void;
  onAccounts: () => void;
  onDetails?: () => void;
}) {
  const [anchor, setAnchor] = createSignal<HTMLElement>();
  const [mode, setMode] = createSignal<UsageMode>("detailed");
  return (
    <>
      <UsageReadout usages={props.usages} now={props.now} refreshing={props.refreshing} onRefresh={props.onRefresh}
        open={anchor() !== undefined} onToggle={(element) => setAnchor(anchor() ? undefined : element)} />
      <Show when={anchor()}>
        {(element) => (
          <Popover anchor={element()} label={t("usage.title")} onClose={() => setAnchor(undefined)}>
            <UsagePanel usages={props.usages} now={props.now} mode={mode()} onMode={setMode} refreshing={props.refreshing}
              onRefresh={props.onRefresh}
              onAccounts={() => { setAnchor(undefined); props.onAccounts(); }}
              onDetails={props.onDetails ? () => { setAnchor(undefined); props.onDetails?.(); } : undefined} />
          </Popover>
        )}
      </Show>
    </>
  );
}
