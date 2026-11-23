import { For, Show, createResource, onMount } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { elapsedMs, formatElapsed, loadLastQaOutcome, loadTaskProfiles } from "../lib/compare-results";
import { chipOf } from "../lib/pr-status";
import { prReport, refreshPrStatus } from "../lib/pr-status-store";
import { shortBranch, type TaskRow } from "../lib/tasks";
import { AgentChip } from "./ui/AgentChip";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { DiffStat } from "./ui/DiffStat";
import { PrChip } from "./ui/PrChip";
import "./CompareResults.css";

const QA_LABEL: Record<string, MessageKey> = {
  passed: "inspector.qa.outcome.passed",
  failed: "inspector.qa.outcome.failed",
  timedOut: "inspector.qa.outcome.timedOut",
  outputLimit: "inspector.qa.outcome.outputLimit",
  processError: "inspector.qa.outcome.processError",
};

interface Details {
  profiles: Record<string, string>;
  qa: Record<string, string | null | undefined>;
}

async function loadDetails(ids: string[]): Promise<Details> {
  const [profiles, outcomes] = await Promise.all([
    loadTaskProfiles(ids).catch(() => ({})),
    Promise.all(ids.map((id) => loadLastQaOutcome(id).catch(() => null))),
  ]);
  return { profiles, qa: Object.fromEntries(ids.map((id, index) => [id, outcomes[index]])) };
}

/**
 * The tasks of one comparison side by side: who ran it, for how long, what changed, whether QA
 * passed and where the pull request stands. Everything shown is already known to the app.
 */
export function CompareResults(props: {
  tasks: readonly TaskRow[];
  now: number;
  onKeep: (task: TaskRow) => void;
  onOpen: (id: string) => void;
  onClose: () => void;
}) {
  const ids = () => props.tasks.map((task) => task.id);
  const [details] = createResource(ids, loadDetails);
  onMount(() => { for (const id of ids()) void refreshPrStatus(id); });

  return (
    <Dialog title={t("compareResults.title")} size="settings" onClose={props.onClose} closeOnScrim escHint>
      <div class="compare-cols" style={{ "--compare-count": String(props.tasks.length) }}>
        <For each={props.tasks}>{(task) => {
          const profile = () => details()?.profiles[task.id];
          const qa = () => details()?.qa[task.id];
          const elapsed = () => elapsedMs(task, props.now);
          const chip = () => chipOf(prReport(task.id));
          return (
            <section class="compare-col" aria-label={task.title}>
              <header>
                <h2 title={task.title}>{task.title}</h2>
                <span class="compare-who">
                  <AgentChip agent={task.agent} />
                  <Show when={profile()}>{(name) => <span class="compare-profile">{name()}</span>}</Show>
                </span>
                <span class="compare-branch">{shortBranch(task.branch)}</span>
              </header>
              <dl class="kv2">
                <dt>{t("compareResults.elapsed")}</dt>
                <dd>{elapsed() === undefined ? "—" : formatElapsed(elapsed() ?? 0)}</dd>
                <dt>{t("compareResults.changes")}</dt>
                <dd>
                  <DiffStat additions={task.additions} deletions={task.deletions} />
                  <Show when={task.additions === 0 && task.deletions === 0}>{t("compareResults.noChanges")}</Show>
                </dd>
                <dt>{t("compareResults.qa")}</dt>
                <dd>
                  <Show when={details()} fallback="—">
                    {qa() ? t(QA_LABEL[qa() ?? ""] ?? "inspector.qa.outcome.processError") : t("compareResults.qaNone")}
                  </Show>
                </dd>
                <dt>{t("compareResults.pullRequest")}</dt>
                <dd>
                  <Show when={chip()} fallback={t("compareResults.noPullRequest")}>
                    {(value) => <PrChip chip={value()} url={prReport(task.id)?.snapshot?.url} />}
                  </Show>
                </dd>
              </dl>
              <footer>
                <Button size="sm" variant="ghost" onClick={() => props.onOpen(task.id)}>{t("board.open")}</Button>
                <Button size="sm" variant="primary" onClick={() => props.onKeep(task)}>{t("task.keepOnlyConfirm")}</Button>
              </footer>
            </section>
          );
        }}</For>
      </div>
    </Dialog>
  );
}
