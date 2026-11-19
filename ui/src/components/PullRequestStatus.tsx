import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import {
  openExternalLink,
  shortSha,
  type PrStatusReport,
  type PullRequestSnapshot,
} from "../lib/pr-status";
import { prFailure, prRefreshing, prReport, refreshPrStatus } from "../lib/pr-status-store";
import { formatAge } from "../lib/time";

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

const STATE_LABEL = { open: "prStatus.open", merged: "prStatus.merged", closed: "prStatus.closed" } as const;

/** Where the task's pull request stands: checks, review, conflicts. Read only. */
export function PullRequestStatus(props: { taskId: string }) {
  // The answer is shared with the task list, which keeps it fresh in the background.
  const report = () => prReport(props.taskId);
  const failure = () => prFailure(props.taskId) ?? "";
  const busy = () => prRefreshing(props.taskId);
  const [now, setNow] = createSignal(Date.now());
  const refresh = (force: boolean) => refreshPrStatus(props.taskId, force).then(() => setNow(Date.now()));

  onMount(() => {
    void refresh(false);
    const clock = setInterval(() => setNow(Date.now()), 30_000);
    onCleanup(() => clearInterval(clock));
  });

  const age = (at: number) => formatAge(at, now());
  const link = (url: string, label: string) => (
    <button type="button" class="link-button" onClick={() => void openExternalLink(url).catch(() => {})}>
      {label}
    </button>
  );

  return (
    <section class="pr-status" aria-label={t("prStatus.title")}>
      <h3>
        {t("prStatus.title")}
        <button type="button" disabled={busy()} onClick={() => void refresh(true)}>
          {t("prStatus.refresh")}
        </button>
      </h3>
      <Show when={failure()}>{(message) => <p role="alert">{message()}</p>}</Show>
      <Show when={!report() && !failure()}><p role="status">{t("prStatus.checking")}</p></Show>
      <Show when={report()}>
        {(current) => (
          <>
            <Show when={current().error}>
              {(error) => (
                <p role="status" class="pr-offline">
                  <strong>{t("prStatus.offlineTitle")}</strong>{" "}
                  {current().snapshot && current().checkedAtMs !== null
                    ? t("prStatus.offlineShowing").replace("{age}", age(current().checkedAtMs ?? 0))
                    : error()}
                  <Show when={current().retryInSeconds}>
                    {(seconds) => <> {t("prStatus.retryIn").replace("{seconds}", String(seconds()))}</>}
                  </Show>
                </p>
              )}
            </Show>
            <Show when={current().noPullRequest}><p class="muted">{t("prStatus.noPullRequest")}</p></Show>
            <Show when={current().snapshot}>
              {(snapshot) => <Snapshot snapshot={snapshot()} report={current()} link={link} />}
            </Show>
            <Show when={current().checkedAtMs !== null && !current().error}>
              <p class="muted">{t("prStatus.checkedAt").replace("{age}", age(current().checkedAtMs ?? 0))}</p>
            </Show>
          </>
        )}
      </Show>
    </section>
  );
}

function Snapshot(props: {
  snapshot: PullRequestSnapshot;
  report: PrStatusReport;
  link: (url: string, label: string) => unknown;
}) {
  const checks = () => props.snapshot.checks;
  return (
    <div class="pr-status-body">
      <p>
        {props.link(props.snapshot.url, `${props.snapshot.repository} #${props.snapshot.number}`) as never}{" "}
        <span class="pr-state">
          {t(STATE_LABEL[props.snapshot.state])}
          <Show when={props.snapshot.draft}> · {t("prStatus.draft")}</Show>
        </span>
      </p>
      <p classList={{ "pr-bad": checks().overall === "failure" }}>
        <strong>{t(CHECKS_LABEL[checks().overall])}</strong>
        <Show when={checks().overall !== "none"}>
          {" "}
          {t("prStatus.checkCounts")
            .replace("{failing}", String(checks().failing))
            .replace("{pending}", String(checks().pending))
            .replace("{passing}", String(checks().passing))}
        </Show>
      </p>
      <Show when={props.snapshot.failedChecks.length > 0}>
        <ul class="pr-failed-checks">
          <For each={props.snapshot.failedChecks}>{(check) => (
            <li>
              <code>{check.name}</code>
              <Show when={check.url}>
                {(url) => <> {props.link(url(), t("prStatus.openLog")) as never}</>}
              </Show>
            </li>
          )}</For>
        </ul>
      </Show>
      <p>{t(REVIEW_LABEL[props.snapshot.review])}</p>
      <p classList={{ "pr-bad": props.snapshot.conflict === "conflicting" }}>
        {t(CONFLICT_LABEL[props.snapshot.conflict])} <code>{props.snapshot.baseBranch}</code>
      </p>
      <p class="muted">
        {t("prStatus.commit").replace("{sha}", shortSha(props.snapshot.headSha))}
        <Show when={props.report.headMatchesLocal === false && props.report.localHead}>
          {(local) => <> {t("prStatus.localDiffers").replace("{local}", shortSha(local()))}</>}
        </Show>
      </p>
    </div>
  );
}
