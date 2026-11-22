import { Input, TextArea } from "./ui/Input";
import { Button } from "./ui/Button";
import { Tooltip } from "./ui/Tooltip";
import { For, Show, createSignal, type JSX } from "solid-js";
import { t } from "../i18n";
import { requestShell } from "../lib/shell-request";
import { pushToast } from "../lib/toasts";
import { holdMinimum } from "../lib/async-action";
import { openExternalLink } from "../lib/pr-status";
import { prReport, refreshPrStatus } from "../lib/pr-status-store";
import type { DiffFile } from "../lib/diff";
import type { GitFailure } from "../lib/git-failure";
import type { GitRunner } from "../lib/task-git-runner";
import { FileRow } from "./FileRow";
import { InspectorNote, OutputLog } from "./InspectorNote";
import { ActionRow, InspectorSection } from "./InspectorSection";

/** A disabled control cannot be hovered, so its reason sits on a wrapper. */
function Reason(props: { text?: string; children: JSX.Element }) {
  return (
    <Show when={props.text} fallback={props.children}>
      {(text) => <Tooltip label={text()}>{(tip) => <span {...tip} class="reason-wrap">{props.children}</span>}</Tooltip>}
    </Show>
  );
}

export type Outcome = { ok: string } | { failure: GitFailure } | undefined;

const failureOf = (value: Outcome) => (value && "failure" in value ? value.failure : undefined);

export function TaskGitActions(props: {
  taskId: string;
  branch?: string;
  runner: GitRunner;
  files: readonly DiffFile[];
  staged: readonly string[];
  untracked: readonly string[];
  conflicts: readonly string[];
  onDraft: (text: string) => Promise<boolean>;
  /** What the sections show before the person acts, for the component catalog. */
  initial?: {
    message?: string;
    prTitle?: string;
    prBody?: string;
    asked?: "commit" | "pr";
    commit?: Outcome;
    push?: Outcome;
    pr?: Outcome;
  };
}) {
  const [message, setMessage] = createSignal(props.initial?.message ?? "");
  const [prTitle, setPrTitle] = createSignal(props.initial?.prTitle ?? "");
  const [prBody, setPrBody] = createSignal(props.initial?.prBody ?? "");
  const [requesting, setRequesting] = createSignal(false);
  const [asked, setAsked] = createSignal<"commit" | "pr" | undefined>(props.initial?.asked);
  const [commitOutcome, setCommitOutcome] = createSignal<Outcome>(props.initial?.commit);
  const [pushOutcome, setPushOutcome] = createSignal<Outcome>(props.initial?.push);
  const [prOutcome, setPrOutcome] = createSignal<Outcome>(props.initial?.pr);
  const [working, setWorking] = createSignal<string | undefined>();
  const busy = () => props.runner.busy();
  const blocked = () => props.conflicts.length > 0;
  const commitReason = () => (blocked() ? t("git.conflictsBlock") : !message().trim() ? t("git.reasonMessage") : undefined);
  const prReason = () => (failureOf(prOutcome())?.kind === "exists" ? t("git.prExists") : !prTitle().trim() ? t("git.reasonPrTitle") : undefined);
  const fail = (error: unknown) => pushToast({ tone: "error", title: t("error.title"), message: String(error) });
  const statOf = (path: string) => props.files.find((file) => file.path === path);
  const existing = () => prReport(props.taskId)?.snapshot ?? undefined;

  const askAgent = async (kind: "commit" | "pr") => {
    if (busy() || requesting()) return;
    setRequesting(true);
    setAsked(undefined);
    try {
      const accepted = await props.onDraft(t(kind === "commit" ? "git.agentCommitPrompt" : "git.agentPrPrompt"));
      if (accepted) {
        setAsked(kind);
        pushToast({ tone: "success", message: t("git.agentDraftReady") });
      } else pushToast({ tone: "error", message: t("diff.notReady") });
    } catch (error) {
      fail(error);
    } finally {
      setRequesting(false);
    }
  };

  const stage = async (action: "stageFile" | "unstageFile", path: string) => {
    setWorking(path);
    try {
      const result = await holdMinimum(props.runner.run({ action, path }));
      if (result && !result.ok) fail(result.failure.output);
    } finally {
      setWorking(undefined);
    }
  };

  const stageTracked = async () => {
    setWorking("stage");
    try {
      const result = await holdMinimum(props.runner.run({ action: "stageTracked" }));
      if (result && !result.ok) fail(result.failure.output);
    } finally {
      setWorking(undefined);
    }
  };

  const commit = async () => {
    if (!message().trim() || blocked()) return;
    setAsked(undefined);
    setCommitOutcome(undefined);
    const result = await props.runner.run({ action: "commit", message: message() });
    if (!result) return;
    if (result.ok) {
      setMessage("");
      setCommitOutcome({ ok: t("git.completed") });
    } else setCommitOutcome({ failure: result.failure });
  };

  const push = async () => {
    setPushOutcome(undefined);
    setWorking("push");
    const result = await holdMinimum(props.runner.run({ action: "push" })).finally(() => setWorking(undefined));
    if (!result) return;
    setPushOutcome(result.ok
      ? { ok: t("git.pushed").replace("{branch}", props.branch ?? "") }
      : { failure: result.failure });
  };

  const createPr = async () => {
    if (!prTitle().trim()) return;
    setAsked(undefined);
    setPrOutcome(undefined);
    const result = await props.runner.run({ action: "createPr", title: prTitle(), body: prBody() });
    if (!result) return;
    if (result.ok) {
      setPrTitle("");
      setPrBody("");
      void refreshPrStatus(props.taskId, true);
    } else {
      setPrOutcome({ failure: result.failure });
      if (result.failure.kind === "exists") void refreshPrStatus(props.taskId, true);
    }
  };

  const existingAddress = (failure: GitFailure) => failure.pullRequestUrl ?? existing()?.url;
  const existingNumber = (failure: GitFailure) => failure.pullRequestNumber ?? existing()?.number;

  const outcome = (value: Outcome, failureNote: (failure: GitFailure) => JSX.Element) => (
    <>
      <Show when={value && "ok" in value ? value.ok : undefined}>{(text) => <InspectorNote tone="ok">{text()}</InspectorNote>}</Show>
      <Show when={value && "failure" in value ? value.failure : undefined}>{(failure) => failureNote(failure())}</Show>
    </>
  );

  return (
    <>
      <InspectorSection title={t("git.staged")}>
        <Show when={props.staged.length} fallback={<InspectorNote>{t("git.nothingStaged")}</InspectorNote>}>
          <For each={props.staged}>{(path) => (
            <FileRow mark={statOf(path)?.status ?? "M"} path={path}
              additions={statOf(path)?.additions} deletions={statOf(path)?.deletions}
              actionLabel={t("git.unstageFile")} actionDisabled={busy()} actionBusy={working() === path}
              onAction={() => void stage("unstageFile", path)} />
          )}</For>
        </Show>
        <ActionRow>
          <Button size="sm" busy={working() === "stage"} disabled={busy()} onClick={() => void stageTracked()}>{t("git.stageTracked")}</Button>
        </ActionRow>
      </InspectorSection>
      <Show when={props.untracked.length > 0}>
        <InspectorSection title={t("inspector.untracked")}>
          <For each={props.untracked}>{(path) => (
            <FileRow mark="U" path={path} actionLabel={t("git.stageFile")} actionDisabled={busy()} actionBusy={working() === path}
              onAction={() => void stage("stageFile", path)} />
          )}</For>
        </InspectorSection>
      </Show>
      <InspectorSection title={t("git.commitSection")}>
        <Show when={blocked()}><InspectorNote>{t("git.conflictsBlock")}</InspectorNote></Show>
        <Show when={asked() === "commit"}><InspectorNote>{t("git.agentDraftReady")}</InspectorNote></Show>
        <TextArea label={t("git.message")} rows={2} disabled={busy() || blocked()}
          value={message()} onInput={(event) => setMessage(event.currentTarget.value)} />
        {outcome(commitOutcome(), (failure) => (
          <>
            <InspectorNote tone="error">{t(failure.kind === "hook" ? "git.commitHook" : "git.commitFailed")}</InspectorNote>
            <OutputLog text={failure.output} />
          </>
        ))}
        <ActionRow>
          <Reason text={commitReason()}>
            <Button variant="primary" size="sm" disabled={!message().trim() || blocked()} loading={busy()}
              onClick={() => void commit()}>{t("git.commit")}</Button>
          </Reason>
          <Button size="sm" icon="bot" disabled={busy() || requesting() || blocked()}
            onClick={() => void askAgent("commit")}>{t("git.askCommit")}</Button>
          <Show when={failureOf(commitOutcome())}>
            {(failure) => (
              <Button size="sm" onClick={() => void props.onDraft(failure().output).then((accepted) => {
                if (!accepted) pushToast({ tone: "error", message: t("diff.notReady") });
              }, fail)}>{t("git.insertOutput")}</Button>
            )}
          </Show>
        </ActionRow>
        <Show when={failureOf(commitOutcome())}>
          <InspectorNote>{t("git.commitKept")}</InspectorNote>
        </Show>
      </InspectorSection>
      <InspectorSection title={t("git.publish")}>
        {outcome(pushOutcome(), (failure) => (
          <>
            <InspectorNote tone="error">
              {failure.kind === "rejected" ? t("git.pushRejected")
                : failure.kind === "denied"
                  ? t("git.pushDenied").replace("{repository}", failure.repository ?? t("git.thisRepository"))
                  : t("git.pushFailed")}
            </InspectorNote>
            <OutputLog text={failure.output} />
            <Show when={failure.kind === "rejected"}><InspectorNote>{t("git.pushRejectedFix")}</InspectorNote></Show>
            <Show when={failure.kind === "denied"}><InspectorNote>{t("git.pushDeniedFix")}</InspectorNote></Show>
          </>
        ))}
        <ActionRow>
          <Button size="sm" icon="download" busy={working() === "push"} disabled={busy()} onClick={() => void push()}>{t("git.push")}</Button>
          <Show when={failureOf(pushOutcome())?.kind === "rejected"}>
            <Button size="sm" icon="terminal" onClick={() => requestShell(props.taskId)}>{t("git.openShell")}</Button>
          </Show>
        </ActionRow>
        <Show when={!existing() || existing()?.state !== "open"}>
          <form class="insp-form" onSubmit={(event) => {
            event.preventDefault();
            void createPr();
          }}>
            <Show when={asked() === "pr"}><InspectorNote>{t("git.agentDraftReady")}</InspectorNote></Show>
            <Input label={t("git.prTitle")} type="text" disabled={busy()}
              value={prTitle()} onInput={(event) => setPrTitle(event.currentTarget.value)} />
            <TextArea label={t("git.prBody")} rows={3} disabled={busy()}
              value={prBody()} onInput={(event) => setPrBody(event.currentTarget.value)} />
            {outcome(prOutcome(), (failure) => (
              <InspectorNote tone="error">
                {t(failure.kind === "exists" ? "git.prExists" : "git.prFailed")}
                <Show when={failure.kind !== "exists"}> {failure.output}</Show>
              </InspectorNote>
            ))}
            <ActionRow>
              <Reason text={prReason()}>
                <Button variant="primary" size="sm" icon="pr" type="submit" loading={busy()}
                  disabled={!prTitle().trim() || failureOf(prOutcome())?.kind === "exists"}>
                  {t("git.createPr")}
                </Button>
              </Reason>
              <Button size="sm" icon="bot" disabled={busy() || requesting()} onClick={() => void askAgent("pr")}>
                {t("git.askPr")}
              </Button>
              <Show when={failureOf(prOutcome())?.kind === "exists" ? failureOf(prOutcome()) : undefined}>
                {(failure) => (
                  <Show when={existingAddress(failure())}>
                    {(address) => (
                      <Button size="sm" onClick={() => void openExternalLink(address()).catch(fail)}>
                        {existingNumber(failure()) ? t("git.openPr").replace("{number}", String(existingNumber(failure()))) : t("task.open")}
                      </Button>
                    )}
                  </Show>
                )}
              </Show>
            </ActionRow>
          </form>
        </Show>
      </InspectorSection>
    </>
  );
}
