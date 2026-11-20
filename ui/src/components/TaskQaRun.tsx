import { Button } from "./ui/Button";
import { Show, createResource, createSignal } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { formatDuration, readLastTaskQa, runTaskQa, shortSha, type QaOutcome, type QaReport } from "../lib/qa-run";

const outcomeLabel: Record<QaOutcome, MessageKey> = {
  passed: "inspector.qa.outcome.passed",
  failed: "inspector.qa.outcome.failed",
  timedOut: "inspector.qa.outcome.timedOut",
  outputLimit: "inspector.qa.outcome.outputLimit",
  processError: "inspector.qa.outcome.processError",
};

/** Runs the project's QA command after approval and shows the last result for the task. */
export function TaskQaRun(props: { taskId: string }) {
  const [last, { mutate }] = createResource(
    () => props.taskId,
    (id) => readLastTaskQa(id).catch(() => null),
  );
  const [running, setRunning] = createSignal(false);
  const [message, setMessage] = createSignal<string>();
  const run = async () => {
    const id = props.taskId;
    setRunning(true);
    setMessage(undefined);
    try {
      const report = await runTaskQa(id);
      if (props.taskId !== id) return;
      if (report) mutate(report);
      else setMessage(t("inspector.qa.declined"));
    } catch (error) {
      if (props.taskId === id) setMessage(String(error));
    } finally {
      setRunning(false);
    }
  };
  const current = (): QaReport | null | undefined => last();
  return (
    <div class="qa-run">
      <Button loading={running()} loadingLabel={t("inspector.qa.waiting")} onClick={() => void run()}>
        {t("inspector.qa.run")}
      </Button>
      <p class="muted">{t("inspector.qa.approvalNote")}</p>
      <Show when={message()}>{(text) => <p role="alert">{text()}</p>}</Show>
      <Show when={current()}>
        {(report) => (
          <div class="qa-result">
            <p>
              <strong classList={{ "qa-passed": report().outcome === "passed", "qa-failed": report().outcome !== "passed" }}>
                {t(outcomeLabel[report().outcome])}
              </strong>
              <Show when={report().exitCode !== null}>
                {" "}· {t("inspector.qa.exit")} {report().exitCode}
              </Show>
              {" "}· {formatDuration(report().durationMs)}
            </p>
            <p class="muted">
              {t("inspector.qa.commit")} <code>{shortSha(report().sha)}</code>
              {" "}· {report().verifiedByBrachio ? t("inspector.qa.verified") : t("inspector.qa.unverified")}
            </p>
            <p class="muted"><code>{report().command}</code></p>
            <p class="muted"><code>{report().directory}</code></p>
            <Show when={report().stdout || report().stderr}>
              <pre class="qa-output" tabindex="0"><code>{(report().stderr + (report().stderr && report().stdout ? "\n" : "") + report().stdout)}</code></pre>
            </Show>
            <Show when={report().truncated}>
              <p class="muted">{t("inspector.qa.truncated")}</p>
            </Show>
          </div>
        )}
      </Show>
    </div>
  );
}
