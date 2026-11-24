import { Show, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { locale, t } from "../../i18n";
import { takeSettingsSection } from "../../lib/settings-scroll";
import {
  RANGES, compact, daysEndingAt, loadProviders, loadReport, perDay, scanInPasses, scanUsage, sumSince, total, utcDay,
  type LimitSnapshot, type Range, type ScanReport, type UsageProviderInfo,
} from "../../lib/usage-analytics";
import { AgentIcon } from "../AgentIcon";
import { BrandIcon, type BrandName } from "../BrandIcon";
import { Button } from "../ui/Button";
import { BarChart, LineChart, ShareList } from "../ui/Chart";
import { SegmentedControl } from "../ui/SegmentedControl";
import "./UsageAnalytics.css";

const ALL = "";
const BRAND_OF: Record<string, BrandName> = { gemini: "gemini", geminiCli: "geminiCli" };
const LINE_COLORS = ["var(--color-accent)", "var(--color-status-working)", "var(--color-status-blocked)", "var(--color-status-ready)"];

/** The mark of a provider; an unknown icon key falls back to the plain bot icon. */
export function ProviderIcon(props: { icon: string }) {
  if (props.icon === "claude") return <AgentIcon agent="claude" />;
  if (props.icon === "codex") return <AgentIcon agent="codex" />;
  if (props.icon === "grok") return <AgentIcon agent="grok" />;
  return <BrandIcon brand={BRAND_OF[props.icon]} fallback="bot" class="icon-sm" />;
}

const windowName = (window: string) => (window === "session" ? t("analytics.window5h") : t("analytics.windowWeek"));
const dayLabel = (day: string) => day.slice(5);

function Card(props: { label: string; value: number }) {
  return (
    <div class="analytics-card">
      <span>{props.label}</span>
      <strong>{compact(props.value)}</strong>
    </div>
  );
}

function limitSeries(limits: readonly LimitSnapshot[], providers: readonly UsageProviderInfo[]) {
  const names = new Map(providers.map((provider) => [provider.id, provider.name]));
  const groups = new Map<string, LimitSnapshot[]>();
  for (const snapshot of limits) {
    const key = `${snapshot.provider}|${snapshot.window}`;
    groups.set(key, [...(groups.get(key) ?? []), snapshot]);
  }
  return [...groups.entries()].map(([key, list], at) => ({
    label: `${names.get(list[0]?.provider ?? "") ?? key.split("|")[0]} · ${windowName(list[0]?.window ?? "")}`,
    color: LINE_COLORS[at % LINE_COLORS.length] ?? "var(--color-accent)",
    points: list.map((snapshot) => ({ at: snapshot.atMs, value: snapshot.usedPercent })),
  }));
}

/** Token statistics read from the agents' local logs: totals, a daily chart, shares, top lists and limit history. */
export function UsageAnalytics() {
  const [provider, setProvider] = createSignal(ALL);
  const [range, setRange] = createSignal<Range>(30);
  const [scan, setScan] = createSignal<ScanReport>();
  const [scanning, setScanning] = createSignal(false);
  const [failure, setFailure] = createSignal("");
  const [providers] = createResource(() => loadProviders().catch(() => []));
  const [report, { refetch }] = createResource(
    () => [provider(), range()] as const,
    ([chosen, days]) => loadReport(chosen === ALL ? undefined : chosen, days),
  );
  const runScan = async (reset: boolean) => {
    setScanning(true);
    setFailure("");
    try {
      await scanInPasses(scanUsage, async (pass) => {
        setScan(pass);
        await refetch();
      }, reset, () => disposed);
    } catch (error) {
      setFailure(String(error));
    } finally {
      setScanning(false);
    }
  };
  let section: HTMLElement | undefined;
  let disposed = false;
  onCleanup(() => { disposed = true; });
  onMount(() => {
    void runScan(false);
    if (takeSettingsSection("analytics")) requestAnimationFrame(() => section?.scrollIntoView({ block: "start" }));
  });

  const today = () => utcDay(Date.now());
  const days = () => daysEndingAt(today(), range());
  const perDays = () => perDay(report()?.daily.filter((row) => row.day >= days()[0]!) ?? [], days());
  const dailyAll = () => report()?.daily ?? [];
  const monthDays = daysEndingAt(today(), 30);
  const nf = new Intl.NumberFormat(locale());
  const names = () => new Map((providers() ?? []).map((item) => [item.id, item]));
  const whenText = (at: number) => new Date(at).toLocaleString(locale(), { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" });
  const shares = (rows: readonly { key: string; name: string; input: number; output: number; cacheRead: number; cacheWrite: number }[]) =>
    rows.map((row) => ({ key: row.key, name: row.name, value: total(row) }));

  return (
    <section class="analytics" aria-labelledby="analytics-title" ref={section}>
      <div class="analytics-head">
        <h3 id="analytics-title">{t("analytics.title")}</h3>
        <Button size="sm" variant="ghost" loading={scanning()} loadingLabel={t("analytics.scanning")}
          onClick={() => void runScan(true)}>{t("analytics.rescan")}</Button>
      </div>
      <div class="analytics-controls">
        <SegmentedControl label={t("analytics.provider")} value={provider()} onChange={setProvider}
          options={[{ value: ALL, label: t("analytics.all") }, ...(providers() ?? []).map((item) => ({ value: item.id, label: item.name }))]} />
        <SegmentedControl label={t("analytics.range")} value={String(range())} onChange={(value) => setRange(Number(value) as Range)}
          options={RANGES.map((days) => ({ value: String(days), label: t("analytics.days").replace("{n}", String(days)) }))} />
      </div>
      <Show when={failure()}><p class="form-error" role="alert">{failure()}</p></Show>
      <Show when={scan()?.truncated && !scanning()}><p class="form-hint">{t("analytics.truncated")}</p></Show>
      <Show when={report()} fallback={<p class="form-hint">{t("analytics.loading")}</p>}>
        {(data) => (
          <Show when={data().daily.length > 0} fallback={<p class="form-hint">{t("analytics.empty")}</p>}>
            <div class="analytics-cards">
              <Card label={t("analytics.today")} value={total(sumSince(dailyAll(), today()))} />
              <Card label={t("analytics.week")} value={total(sumSince(dailyAll(), monthDays[23]!))} />
              <Card label={t("analytics.month")} value={total(sumSince(dailyAll(), monthDays[0]!))} />
            </div>
            <h4>{t("analytics.perDay")}</h4>
            <BarChart label={t("analytics.perDay")} categories={days()} axis={dayLabel} format={(value) => nf.format(Math.round(value))}
              series={[
                { label: t("analytics.input"), color: "var(--color-accent)", values: perDays().map((entry) => entry.tokens.input) },
                { label: t("analytics.output"), color: "var(--color-status-working)", values: perDays().map((entry) => entry.tokens.output) },
                { label: t("analytics.cache"), color: "var(--color-status-neutral)", values: perDays().map((entry) => entry.tokens.cacheRead + entry.tokens.cacheWrite) },
              ]} />
            <h4>{t("analytics.sessionsPerDay")}</h4>
            <BarChart label={t("analytics.sessionsPerDay")} categories={days()} axis={dayLabel} format={(value) => nf.format(value)}
              series={[{ label: t("analytics.sessions"), color: "var(--color-status-ready)", values: perDays().map((entry) => entry.sessions) }]} />
            <div class="analytics-lists">
              <div>
                <h4>{t("analytics.share")}</h4>
                <ShareList label={t("analytics.share")} empty={t("analytics.none")} format={compact}
                  rows={shares(data().byProvider).map((row) => ({ ...row, icon: <ProviderIcon icon={names().get(row.key)?.icon ?? ""} /> }))} />
              </div>
              <div>
                <h4>{t("analytics.topProjects")}</h4>
                <ShareList label={t("analytics.topProjects")} empty={t("analytics.none")} format={compact} rows={shares(data().byProject)} />
              </div>
              <div>
                <h4>{t("analytics.topTasks")}</h4>
                <ShareList label={t("analytics.topTasks")} empty={t("analytics.none")} format={compact} rows={shares(data().byTask)} />
              </div>
            </div>
            <h4>{t("analytics.limits")}</h4>
            <Show when={data().limits.length > 1} fallback={<p class="form-hint">{t("analytics.limitsEmpty")}</p>}>
              <LineChart label={t("analytics.limits")} from={Date.now() - range() * 86_400_000} to={Date.now()}
                series={limitSeries(data().limits.filter((entry) => provider() === ALL || entry.provider === provider()), providers() ?? [])}
                when={whenText} />
            </Show>
            <p class="form-hint">{t("analytics.note")}</p>
          </Show>
        )}
      </Show>
    </section>
  );
}
