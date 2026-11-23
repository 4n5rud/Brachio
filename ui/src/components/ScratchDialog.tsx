import { For, Show, createSignal, onMount } from "solid-js";
import { t } from "../i18n";
import { AGENT_CATALOG, AGENT_KINDS } from "../lib/agent-catalog";
import { createScratchTask } from "../lib/scratch";
import { CORE_AGENT } from "../lib/terminal";
import type { AgentKind } from "../lib/tasks";
import type { CreatedTask } from "../lib/workspace";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Input, Select, TextArea } from "./ui/Input";

/** Starts an agent in a temporary folder with no git repository. */
export function ScratchDialog(props: {
  onStarted: (task: CreatedTask) => void | Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = createSignal("");
  const [prompt, setPrompt] = createSignal("");
  const [agent, setAgent] = createSignal<AgentKind>("claude");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  let first!: HTMLTextAreaElement;
  onMount(() => first.focus());

  const submit = async () => {
    if (busy()) return;
    setBusy(true);
    setError(undefined);
    try {
      const task = await createScratchTask({ title: name().trim(), prompt: prompt().trim(), agent: CORE_AGENT[agent()] });
      await props.onStarted(task);
    } catch (failure) {
      setError(String(failure));
      setBusy(false);
    }
  };

  return (
    <Dialog title={t("scratch.title")} onClose={() => { if (!busy()) props.onClose(); }} escHint initialFocus={() => first}
      footer={
        <>
          <Button variant="ghost" disabled={busy()} onClick={props.onClose}>{t("newTask.cancel")}</Button>
          <Button variant="primary" loading={busy()} loadingLabel={t("scratch.starting")} onClick={() => void submit()}>
            {t("scratch.start")}
          </Button>
        </>
      }>
      <div class="form-field">
        <p class="form-hint">{t("scratch.note")}</p>
        <TextArea ref={first} label={t("scratch.prompt")} rows={5} value={prompt()} disabled={busy()}
          onInput={(event) => setPrompt(event.currentTarget.value)} />
        <Input label={t("scratch.name")} value={name()} disabled={busy()} onInput={(event) => setName(event.currentTarget.value)} />
        <Select label={t("newTask.agent")} value={agent()} disabled={busy()}
          onChange={(event) => setAgent(event.currentTarget.value as AgentKind)}>
          <For each={AGENT_KINDS}>{(kind) => <option value={kind}>{AGENT_CATALOG[kind].name}</option>}</For>
        </Select>
      </div>
      <Show when={error()}>
        {(message) => (
          <div class="create-failed" role="alert">
            <strong>{t("scratch.failed")}</strong>
            <pre class="create-log">{message()}</pre>
          </div>
        )}
      </Show>
    </Dialog>
  );
}
