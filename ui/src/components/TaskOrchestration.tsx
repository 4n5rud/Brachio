import { Toggle } from "./ui/Toggle";
import { Input } from "./ui/Input";
import { Button } from "./ui/Button";
import { For, Show, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import { shortSha } from "../lib/qa-run";
import {
  readOrchestration,
  reopenChild,
  reviewHandoff,
  setScheduler,
  testEvidence,
  watchOrchestration,
  type ChildView,
} from "../lib/orchestration";
import { PlanGraph } from "./PlanGraph";
import { StateChip, stateText } from "./OrchestrationState";

function ChildCard(props: { child: ChildView; onChanged: () => void }) {
  const [rejecting, setRejecting] = createSignal(false);
  const [reason, setReason] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const decide = async (handoffId: string, approved: boolean) => {
    setBusy(true);
    setError(undefined);
    try {
      await reviewHandoff(handoffId, approved, approved ? undefined : reason().trim());
      setRejecting(false);
      setReason("");
      props.onChanged();
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  };
  return (
    <li class="orchestration-child">
      <div class="orchestration-child-head">
        <strong>{props.child.title}</strong>
        <StateChip state={props.child.state} />
      </div>
      <Show when={props.child.reason}>{(reason) => <p class="muted">{reason()}</p>}</Show>
      <Show when={props.child.startBlocked}>
        <Button
          type="button"
          disabled={busy()}
          onClick={async () => {
            setBusy(true);
            setError(undefined);
            try {
              await reopenChild(props.child.id);
              props.onChanged();
            } catch (failure) {
              setError(String(failure));
            } finally {
              setBusy(false);
            }
          }}
        >
          {t("orchestration.reopen")}
        </Button>
      </Show>
      <Show when={props.child.dependencies.length > 0}>
        <p class="muted">
          {t("orchestration.dependsOn")}{" "}
          <For each={props.child.dependencies}>
            {(dependency, index) => (
              <>
                <Show when={index() > 0}>{", "}</Show>
                {dependency.title}
                {" ("}
                {stateText(dependency.state)}
                {")"}
              </>
            )}
          </For>
        </p>
      </Show>
      <Show when={props.child.handoff} fallback={<p class="muted">{t("orchestration.noHandoff")}</p>}>
        {(handoff) => (
          <div class="orchestration-handoff">
            <p>
              {t("orchestration.commit")} <code>{shortSha(handoff().commitSha)}</code>
              {" · "}
              {handoff().changedPathsCount} {t("orchestration.files")}
            </p>
            <p class="orchestration-claim">{handoff().summary}</p>
            <Show when={handoff().changedPaths.length > 0}>
              <details>
                <summary>{t("orchestration.changedFiles")}</summary>
                <ul>
                  <For each={handoff().changedPaths}>{(path) => <li><code>{path}</code></li>}</For>
                </ul>
              </details>
            </Show>
            <p class="muted">
              {t("orchestration.reportedTests")}:{" "}
              {handoff().reportedTests.length > 0 ? handoff().reportedTests.join(", ") : t("orchestration.none")}
              {" "}({t("orchestration.agentClaim")})
            </p>
            <p classList={{ muted: testEvidence(handoff()) === "none", "qa-passed": testEvidence(handoff()) === "verified", "qa-failed": testEvidence(handoff()) === "failed" }}>
              {t("orchestration.verifiedQa")}:{" "}
              {testEvidence(handoff()) === "verified" ? t("orchestration.qa.passed")
                : testEvidence(handoff()) === "failed" ? t("orchestration.qa.failed")
                : t("orchestration.qa.none")}
            </p>
            <Show when={handoff().verifiedQa}>
              {(qa) => (
                <p class="muted">
                  <code>{qa().command}</code>
                  {" · "}
                  {t("orchestration.exitCode")} {qa().exitCode ?? "–"}
                  {" · "}
                  <code>{shortSha(handoff().commitSha)}</code>
                  {" · "}
                  {t("orchestration.verifiedByBrachio")}
                </p>
              )}
            </Show>
            <Show when={handoff().review}>
              {(review) => (
                <p class="muted">
                  {review() === "approved" ? t("orchestration.approved") : t("orchestration.rejected")}
                  <Show when={handoff().reviewNote}>{(note) => <>{": "}{note()}</>}</Show>
                </p>
              )}
            </Show>
            <Show when={handoff().canReview}>
              <div class="orchestration-actions">
                <Button variant="primary" disabled={busy()}
                  onClick={() => void decide(handoff().id, true)}>
                  {t("orchestration.approve")}
                </Button>
                <Button disabled={busy()} onClick={() => setRejecting((open) => !open)}>
                  {t("orchestration.reject")}
                </Button>
              </div>
              <Show when={rejecting()}>
                <form class="orchestration-reject" onSubmit={(event) => {
                  event.preventDefault();
                  void decide(handoff().id, false);
                }}>
                  <Input type="text" maxLength={300} value={reason()}
                    label={t("orchestration.reasonLabel")}
                    placeholder={t("orchestration.reasonPlaceholder")}
                    onInput={(event) => setReason(event.currentTarget.value)} />
                  <Button type="submit" disabled={busy() || reason().trim() === ""}>
                    {t("orchestration.confirmReject")}
                  </Button>
                </form>
              </Show>
            </Show>
            <Show when={error()}>{(text) => <p role="alert">{text()}</p>}</Show>
          </div>
        )}
      </Show>
    </li>
  );
}

/** The lead's child tasks with their dependencies and handoffs; completing one needs the user's approval. */
export function TaskOrchestration(props: { leadTaskId: string }) {
  const [view, { refetch }] = createResource(
    () => props.leadTaskId,
    (id) => readOrchestration(id).catch((error) => ({ error: String(error) })),
  );
  const [schedulerError, setSchedulerError] = createSignal<string>();
  onMount(() => onCleanup(watchOrchestration(() => void refetch())));
  const current = () => {
    const value = view();
    return value && "children" in value ? value : undefined;
  };
  const failure = () => {
    const value = view();
    return value && "error" in value ? value.error : undefined;
  };
  const [selectedId, setSelectedId] = createSignal<string>();
  // Opens on the first task waiting for the user's review, else the first task.
  const selected = () => {
    const children = current()?.children ?? [];
    return children.find((child) => child.id === selectedId())
      ?? children.find((child) => child.handoff?.canReview)
      ?? children[0];
  };
  return (
    <div class="orchestration">
      <p class="muted">{t("orchestration.note")}</p>
      <Show when={failure()}>{(text) => <p role="alert">{text()}</p>}</Show>
      <Show when={current()}>
        {(data) => (
          <>
          <Toggle checked={data().schedulerEnabled} label={t("orchestration.scheduler")}
            onChange={(enabled) => {
              setScheduler(props.leadTaskId, enabled).then(() => refetch(), (error) => {
                setSchedulerError(String(error));
                void refetch();
              });
            }} />
          <p class="muted">{t("orchestration.schedulerNote")} ({data().maxRunning})</p>
          <Show when={schedulerError()}>{(text) => <p role="alert">{text()}</p>}</Show>
          <Show when={data().children.length > 0} fallback={<p class="muted">{t("orchestration.empty")}</p>}>
            <PlanGraph children={data().children} selectedId={selected()?.id} onSelect={setSelectedId} />
            <p class="muted">{t("orchestration.graphNote")}</p>
            <Show when={selected()} keyed>
              {(child) => (
                <ul class="orchestration-list">
                  <ChildCard child={child} onChanged={() => void refetch()} />
                </ul>
              )}
            </Show>
          </Show>
          </>
        )}
      </Show>
    </div>
  );
}
