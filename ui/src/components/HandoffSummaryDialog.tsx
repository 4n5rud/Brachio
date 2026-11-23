import { For, Show, createEffect, createSignal, on } from "solid-js";
import { t } from "../i18n";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Notice } from "./ui/Notice";
import { Select, TextArea } from "./ui/Input";
import { buildHandoffSummary, sendHandoffSummary, utf8Length } from "../lib/handoff-summary";

/**
 * Hands another task's context to this task's agent. The summary is built from the source task,
 * edited by the user, and only then sent to the mailbox or drafted into the input. Nothing goes
 * out until a button is pressed.
 */
export function HandoffSummaryDialog(props: {
  /** The agent tab that receives the summary. */
  targetKey: string;
  sources: readonly { id: string; title: string }[];
  initialSourceId: string;
  /** Inserts text into the agent input without sending it. */
  onDraft: (text: string) => Promise<boolean>;
  onClose: () => void;
}) {
  const [sourceId, setSourceId] = createSignal(props.initialSourceId);
  const [text, setText] = createSignal("");
  const [maxBytes, setMaxBytes] = createSignal(7000);
  const [loading, setLoading] = createSignal(true);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  createEffect(on(sourceId, (id) => {
    setLoading(true);
    setError(undefined);
    void buildHandoffSummary(id)
      .then((summary) => { if (sourceId() === id) { setText(summary.text); setMaxBytes(summary.maxBytes); } })
      .catch((failure) => { if (sourceId() === id) setError(String(failure)); })
      .finally(() => { if (sourceId() === id) setLoading(false); });
  }));
  const size = () => utf8Length(text());
  const tooLong = () => size() > maxBytes();
  const canSend = () => !loading() && !busy() && text().trim() !== "" && !tooLong();
  const requestId = crypto.randomUUID();
  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(undefined);
    try {
      await action();
      props.onClose();
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("handoff.title")} size="sheet" onClose={props.onClose}
      footer={<>
        <Button variant="ghost" onClick={props.onClose}>{t("confirm.cancel")}</Button>
        <Button disabled={!canSend()} onClick={() => void run(async () => {
          if (!(await props.onDraft(text()))) throw new Error(t("diff.notReady"));
        })}>{t("handoff.draft")}</Button>
        <Button variant="primary" disabled={!canSend() || sourceId() === ""} onClick={() => void run(() =>
          sendHandoffSummary(sourceId(), props.targetKey, text(), requestId))}>{t("handoff.send")}</Button>
      </>}>
      <p class="muted">{t("handoff.note")}</p>
      <Select label={t("handoff.source")} value={sourceId()} onChange={(event) => setSourceId(event.currentTarget.value)}>
        <For each={props.sources}>{(source) => <option value={source.id}>{source.title}</option>}</For>
      </Select>
      <TextArea label={t("handoff.text")} rows={16} mono value={text()} disabled={loading()}
        invalid={tooLong()} onInput={(event) => setText(event.currentTarget.value)}
        hint={`${size()} / ${maxBytes()}`} />
      <Show when={error()}>{(message) => <Notice tone="error" lead={t("error.title")}>{message()}</Notice>}</Show>
    </Dialog>
  );
}
