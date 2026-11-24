import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { visibleInterval } from "../lib/visible-interval";
import { t } from "../i18n";
import { useAsyncAction } from "../lib/async-action";
import {
  chipOfSnapshot,
  linkPr,
  openExternalLink,
  shortSha,
  unlinkPr,
  type LinkedPrReport,
  type PrChip as PrChipState,
  type PrStatusReport,
  type PullRequestSnapshot,
} from "../lib/pr-status";
import { prFailure, prRefreshing, prReport, refreshPrStatus } from "../lib/pr-status-store";
import { formatAge } from "../lib/time";
import { PrChip } from "./ui/PrChip";
import { Button } from "./ui/Button";
import { Input } from "./ui/Input";
import { IconButton } from "./ui/IconButton";
import { Icon, type IconName } from "./Icon";
import { FileRow } from "./FileRow";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import "./PullRequestStatus.css";

const REVIEW_LABEL = {
  approved: "prStatus.reviewApproved",
  changes_requested: "prStatus.reviewChanges",
  review_required: "prStatus.reviewRequired",
  not_required: "prStatus.reviewNone",
} as const;

const CONFLICT_LABEL = {
  clean: "prStatus.conflictClean",
  conflicting: "prStatus.conflicting",
  unknown: "prStatus.conflictUnknown",
} as const;

const CHECKS_LABEL = {
  failure: "prStatus.checksFailure",
  pending: "prStatus.checksPending",
  success: "prStatus.checksSuccess",
  none: "prStatus.checksNone",
} as const;

const CHECKS_TONE = { failure: "error", pending: "working", success: "ready", none: undefined } as const;

const CHECKS_ICON: Record<string, IconName> = { failure: "ghX", pending: "ghDot", success: "ghCheck" };

const STATE_KEY = { open: "prChip.open", merged: "prChip.merged", closed: "prChip.closed" } as const;

/** Where the task's pull request stands: checks, review, conflicts. Read only. */
export function PullRequestStatus(props: { taskId: string }) {
  // The answer is shared with the task list, which keeps it fresh in the background.
  const [now, setNow] = createSignal(Date.now());
  const refresh = (force: boolean) => refreshPrStatus(props.taskId, force).then(() => setNow(Date.now()));
  const manual = useAsyncAction(() => refresh(true));

  onMount(() => {
    void refresh(false);
    onCleanup(visibleInterval(() => setNow(Date.now()), 30_000));
  });

  return (
    <PullRequestPanel report={prReport(props.taskId)} failure={prFailure(props.taskId)} busy={prRefreshing(props.taskId) || manual.pending()} done={manual.done()}
      now={now()} onRefresh={() => void manual.run()}
      onLink={async (reference) => {
        await linkPr(props.taskId, reference);
        await refresh(true);
      }}
      onUnlink={async (link) => {
        await unlinkPr(props.taskId, link.repository, link.number);
        await refresh(true);
      }} />
  );
}

export function PullRequestPanel(props: {
  report?: PrStatusReport;
  failure?: string;
  busy: boolean;
  /** The last manual refresh just finished. */
  done?: boolean;
  now: number;
  onRefresh: () => void;
  /** Links another pull request; rejects with the reason it was refused. Without it the form is hidden. */
  onLink?: (reference: string) => Promise<void>;
  onUnlink?: (link: LinkedPrReport) => Promise<void>;
}) {
  const age = (at: number) => formatAge(at, props.now);

  return (
    <InspectorSection title={t("prStatus.title")}
      end={<IconButton size="sm" icon="refresh" label={t("prStatus.refresh")} busy={props.busy} done={props.done}
        onClick={props.onRefresh} />}>
      <Show when={props.failure}>{(message) => <InspectorNote tone="error">{message()}</InspectorNote>}</Show>
      <Show when={!props.report && !props.failure}>
        <InspectorNote tone="progress">{t("prStatus.checking")}</InspectorNote>
      </Show>
      <Show when={props.report}>
        {(current) => (
          <>
            <Show when={current().error}>
              {(error) => (
                <InspectorNote tone="warn">
                  {t("prStatus.offlineTitle")}{" "}
                  {current().snapshot && current().checkedAtMs !== null
                    ? t("prStatus.offlineShowing").replace("{age}", age(current().checkedAtMs ?? 0))
                    : error()}
                  <Show when={current().retryInSeconds}>
                    {(seconds) => <> {t("prStatus.retryIn").replace("{seconds}", String(seconds()))}</>}
                  </Show>
                </InspectorNote>
              )}
            </Show>
            <Show when={current().noPullRequest}>
              <InspectorNote>{t("prStatus.noPullRequest")}</InspectorNote>
            </Show>
            <Show when={current().snapshot}>
              {(snapshot) => <Snapshot snapshot={snapshot()} report={current()} />}
            </Show>
            <Show when={current().checkedAtMs !== null && !current().error}>
              <InspectorNote>{t("prStatus.checkedAt").replace("{age}", age(current().checkedAtMs ?? 0))}</InspectorNote>
            </Show>
            <For each={current().linked}>
              {(link) => <LinkedRow link={link} onUnlink={props.onUnlink} />}
            </For>
          </>
        )}
      </Show>
      <Show when={props.onLink}>{(onLink) => <LinkForm onLink={onLink()} />}</Show>
    </InspectorSection>
  );
}

function LinkedRow(props: { link: LinkedPrReport; onUnlink?: (link: LinkedPrReport) => Promise<void> }) {
  const unlink = useAsyncAction(async () => props.onUnlink?.(props.link), { failureToast: true });
  const chip = (): PrChipState | null =>
    props.link.snapshot ? chipOfSnapshot(props.link.snapshot, props.link.error !== null) : null;
  const open = (url: string) => void openExternalLink(url).catch(() => {});
  return (
    <div class="row-gap">
      <Show when={chip()}>{(value) => <PrChip chip={value()} url={props.link.snapshot?.url} />}</Show>
      <button type="button" class="link-button"
        onClick={() => open(props.link.snapshot?.url ?? `https://github.com/${props.link.repository}/pull/${props.link.number}`)}>
        {props.link.repository}#{props.link.number}
      </button>
      <Show when={props.link.error}>
        <span class="note note-warn">{props.link.snapshot ? t("prStatus.linkedOffline") : props.link.error}</span>
      </Show>
      <Show when={!props.link.snapshot && !props.link.error}>
        <span class="note">{t("prStatus.linkedUnknown")}</span>
      </Show>
      <Show when={props.onUnlink}>
        <IconButton size="sm" icon="x" label={t("prStatus.unlink").replace("{number}", `#${props.link.number}`)}
          busy={unlink.pending()} onClick={() => void unlink.run()} />
      </Show>
    </div>
  );
}

function LinkForm(props: { onLink: (reference: string) => Promise<void> }) {
  const [reference, setReference] = createSignal("");
  const [error, setError] = createSignal<string>();
  const link = useAsyncAction(async () => {
    setError(undefined);
    try {
      await props.onLink(reference().trim());
      setReference("");
    } catch (failure) {
      setError(String(failure));
    }
  });
  return (
    <form class="row-gap" onSubmit={(event) => { event.preventDefault(); void link.run(); }}>
      <Input compact mono aria-label={t("prStatus.linkLabel")} placeholder={t("prStatus.linkPlaceholder")}
        value={reference()} error={error()} onInput={(event) => setReference(event.currentTarget.value)} />
      <Button type="submit" size="sm" busy={link.pending()} disabled={reference().trim() === ""}>
        {t("prStatus.link")}
      </Button>
    </form>
  );
}

function Snapshot(props: { snapshot: PullRequestSnapshot; report: PrStatusReport }) {
  const checks = () => props.snapshot.checks;
  const chip = (): PrChipState => ({
    key: props.snapshot.draft && props.snapshot.state === "open" ? "prChip.draft" : STATE_KEY[props.snapshot.state],
    number: props.snapshot.number,
    tone: "neutral",
    stale: props.report.error !== null,
  });
  const open = (url: string) => void openExternalLink(url).catch(() => {});
  return (
    <>
      <div class="row-gap">
        <PrChip chip={chip()} url={props.snapshot.url} />
        <button type="button" class="link-button" onClick={() => open(props.snapshot.url)}>
          {props.snapshot.repository}
        </button>
      </div>
      <dl class="kv2">
        <dt>{t("prStatus.checks")}</dt>
        <dd>
          <Show when={CHECKS_TONE[checks().overall]} fallback={t(CHECKS_LABEL[checks().overall])}>
            {(tone) => (
              <span class={`status status-${tone()}`}>
                <Icon name={CHECKS_ICON[checks().overall] ?? "ghDot"} class="icon-xs" />
                {t(CHECKS_LABEL[checks().overall])}
              </span>
            )}
          </Show>
          <Show when={checks().overall !== "none"}>
            {" "}
            {t("prStatus.checkCounts")
              .replace("{failing}", String(checks().failing))
              .replace("{pending}", String(checks().pending))
              .replace("{passing}", String(checks().passing))}
          </Show>
        </dd>
        <dt>{t("prStatus.review")}</dt>
        <dd>{t(REVIEW_LABEL[props.snapshot.review])}</dd>
        <dt>{t("prStatus.merge")}</dt>
        <dd>{t(CONFLICT_LABEL[props.snapshot.conflict])} <Icon name="ghBranch" class="icon-xs" /><code>{props.snapshot.baseBranch}</code></dd>
      </dl>
      <For each={props.snapshot.failedChecks}>{(check) => (
        <FileRow mark="failed" path={check.name} label={check.name}
          actionLabel={check.url ? t("prStatus.openLog") : undefined}
          onAction={() => check.url && open(check.url)} />
      )}</For>
      <InspectorNote><Icon name="ghCommit" class="icon-xs" /> {t("prStatus.commit").replace("{sha}", shortSha(props.snapshot.headSha))}</InspectorNote>
      <Show when={props.report.headMatchesLocal === false && props.report.localHead}>
        {(local) => (
          <InspectorNote tone="warn">
            {t("prStatus.localDiffers")
              .replace("{local}", shortSha(local()))
              .replace("{remote}", shortSha(props.snapshot.headSha))}
          </InspectorNote>
        )}
      </Show>
    </>
  );
}
