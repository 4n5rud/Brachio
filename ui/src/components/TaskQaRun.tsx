import { Button } from "./ui/Button";
import { Show, createResource, createSignal } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { deleteTaskQaOutput, formatDuration, readLastTaskQa, runTaskQa, shortSha, type QaOutcome, type QaReport } from "../lib/qa-run";
import { InspectorNote, OutputLog } from "./InspectorNote";
import { ActionRow, InspectorSection } from "./InspectorSection";
import { StatusGlyph } from "./ui/StatusBadge";

const outcomeLabel: Record<QaOutcome, MessageKey> = {
  passed: "inspector.qa.outcome.passed",
  failed: "inspector.qa.outcome.failed",
  timedOut: "inspector.qa.outcome.timedOut",
  outputLimit: "inspector.qa.outcome.outputLimit",
  processError: "inspector.qa.outcome.processError",
};

const outcomeNote: Partial<Record<QaOutcome, MessageKey>> = {
  timedOut: "inspector.qa.timedOutNote",
  outputLimit: "inspector.qa.outputLimitNote",
};

const reportOutput = (report: QaReport) =>
  report.stderr + (report.stderr && report.stdout ? "\n" : "") + report.stdout;

/** What went wrong in a failed run, ready to send to the agent. */
export const qaFailureDraft = (report: QaReport) =>
  t("inspector.qa.failureDraft")
    .replace("{command}", report.command)
    .replace("{exit}", String(report.exitCode ?? "-"))
    .replace("{commit}", shortSha(report.sha))
    .replace("{output}", reportOutput(report).trimEnd()) + "\n";

/** The last QA run of a task: running, waiting for approval, or what Brachio saw on which commit. */
export function QaPanel(props: {
  report?: QaReport | null;
  running: boolean;
  message?: { tone: "warn" | "error"; text: string };
  onRun: () => void;
  onInsertFailure?: (report: QaReport) => void;
  onDeleteOutput?: () => void;
}) {
  const run = () => (
    <Button size="sm" icon="refresh" loading={props.running} loadingLabel={t("inspector.qa.waiting")} onClick={props.onRun}>
      {t("inspector.qa.run")}
    </Button>
  );
  return (
    <InspectorSection title={props.report ? t("inspector.qa.last") : t("inspector.qa")}
      end={props.report ? run() : undefined}>
      <Show when={props.message}>{(message) => <InspectorNote tone={message().tone}>{message().text}</InspectorNote>}</Show>
      <Show when={props.report} fallback={
        <>
          <Show when={!props.message && !props.running}><InspectorNote>{t("inspector.qa.none")}</InspectorNote></Show>
          <ActionRow>{run()}</ActionRow>
        </>
      }>
        {(report) => (
          <>
            <div class="row-gap">
              <span class={`status status-${report().outcome === "passed" ? "ready" : "error"}`}>
                <StatusGlyph tone={report().outcome === "passed" ? "ready" : "error"} />
                {t(outcomeLabel[report().outcome])}
              </span>
              <Show when={report().exitCode !== null}>
                <span class="count">{t("inspector.qa.exit")} {report().exitCode}</span>
              </Show>
              <span class="count">{formatDuration(report().durationMs)}</span>
            </div>
            <dl class="kv2">
              <dt>{t("inspector.qa.command")}</dt><dd><code>{report().command}</code></dd>
              <dt>{t("inspector.qa.folder")}</dt><dd><code>{report().directory}</code></dd>
              <dt>{t("inspector.qa.commit")}</dt><dd><code>{shortSha(report().sha)}</code></dd>
            </dl>
            <Show when={report().outcome === "passed" || report().outcome === "failed"}>
              <InspectorNote tone={report().verifiedByBrachio ? "ok" : "warn"}>
                {report().verifiedByBrachio ? t("inspector.qa.verified") : t("inspector.qa.unverified")}
              </InspectorNote>
            </Show>
            <Show when={reportOutput(report())}>{(output) => <OutputLog text={output()} />}</Show>
            <Show when={outcomeNote[report().outcome]}>{(key) => <InspectorNote>{t(key())}</InspectorNote>}</Show>
            <Show when={report().truncated}><InspectorNote>{t("inspector.qa.truncated")}</InspectorNote></Show>
            <Show when={props.onDeleteOutput}>
              {(remove) => (
                <ActionRow>
                  <Button size="sm" onClick={() => remove()()}>{t("inspector.qa.deleteOutput")}</Button>
                </ActionRow>
              )}
            </Show>
            <Show when={report().outcome === "failed" && props.onInsertFailure}>
              {(insert) => (
                <ActionRow>
                  <Button size="sm" variant="primary" onClick={() => insert()(report())}>{t("inspector.qa.insertFailure")}</Button>
                </ActionRow>
              )}
            </Show>
          </>
        )}
      </Show>
    </InspectorSection>
  );
}

/** Runs the project's QA command after approval and shows the last result for the task. */
export function TaskQaRun(props: { taskId: string; onDraft: (text: string) => Promise<boolean> }) {
  const [last, { mutate }] = createResource(
    () => props.taskId,
    (id) => readLastTaskQa(id).catch(() => null),
  );
  const [running, setRunning] = createSignal(false);
  const [message, setMessage] = createSignal<{ tone: "warn" | "error"; text: string }>();
  const run = async () => {
    const id = props.taskId;
    setRunning(true);
    setMessage(undefined);
    try {
      const report = await runTaskQa(id);
      if (props.taskId !== id) return;
      if (report) mutate(report);
      else setMessage({ tone: "warn", text: t("inspector.qa.declined") });
    } catch (error) {
      if (props.taskId === id) setMessage({ tone: "error", text: String(error) });
    } finally {
      setRunning(false);
    }
  };
  const deleteOutput = async () => {
    const id = props.taskId;
    try {
      await deleteTaskQaOutput(id);
      if (props.taskId === id) mutate(null);
    } catch (error) {
      if (props.taskId === id) setMessage({ tone: "error", text: String(error) });
    }
  };
  return (
    <QaPanel report={last()} running={running()} message={message()} onRun={() => void run()}
      onDeleteOutput={() => void deleteOutput()}
      onInsertFailure={(report) => void props.onDraft(qaFailureDraft(report))} />
  );
}
