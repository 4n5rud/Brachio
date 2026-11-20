import { Button } from "./ui/Button";
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { confirmDialog } from "./ui/ConfirmDialog";
import { Dialog } from "./ui/Dialog";
import { Input, Select, TextArea } from "./ui/Input";
import { t } from "../i18n";
import { AGENT_NAME, type AgentKind, type TaskRow } from "../lib/tasks";
import { AgentIcon } from "./AgentIcon";
import { SUBMIT_KEYS } from "../lib/shortcut";
import { titleFromPrompt } from "../lib/workspace";

/** What the dialog hands back when the user creates a task. */
export interface NewTaskInput {
  prompt: string;
  title: string;
  agent: AgentKind;
  /** Empty means the repository's default branch. */
  base: string;
  parentTaskId?: string;
}

const AGENTS: readonly AgentKind[] = ["claude", "codex"];

/** "New task" dialog: prompt, name, agent and base branch. */
export function NewTaskDialog(props: {
  defaultBranch: string;
  /** A plain folder has no branch to base a task on and no parent task. */
  isGit: boolean;
  parents?: readonly TaskRow[];
  onCreate: (input: NewTaskInput) => Promise<void>;
  onClose: () => void;
}) {
  const [prompt, setPrompt] = createSignal("");
  const [title, setTitle] = createSignal("");
  const [agent, setAgent] = createSignal<AgentKind>("claude");
  const [base, setBase] = createSignal("");
  const [parentTaskId, setParentTaskId] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [confirmingClose, setConfirmingClose] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const finalTitle = () => title().trim() || titleFromPrompt(prompt());
  const canCreate = () => !busy() && !confirmingClose() && finalTitle() !== "";
  const requestClose = async () => {
    if (busy() || confirmingClose()) return;
    if (!prompt().trim() && !title().trim() && !base().trim() && agent() === "claude") {
      props.onClose();
      return;
    }
    setConfirmingClose(true);
    try {
      const discard = await confirmDialog({
        title: t("newTask.discardDraft"),
        confirmLabel: t("confirm.discard"),
        tone: "danger",
      });
      if (discard) props.onClose();
    } catch (failure) {
      setError(String(failure));
    } finally {
      setConfirmingClose(false);
    }
  };

  const submit = async () => {
    if (!canCreate()) return;
    setBusy(true);
    setError(null);
    try {
      await props.onCreate({ prompt: prompt().trim(), title: finalTitle(), agent: agent(),
        base: base().trim(), parentTaskId: parentTaskId() || undefined });
    } catch (failure) {
      setError(String(failure));
      setBusy(false);
    }
  };

  let promptBox!: HTMLTextAreaElement;
  onMount(() => promptBox.focus());
  const onKey = (event: KeyboardEvent) => {
    if (event.isComposing || event.defaultPrevented) return;
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) void submit();
  };
  window.addEventListener("keydown", onKey);
  onCleanup(() => window.removeEventListener("keydown", onKey));

  return (
    <Dialog
      title={t("newTask.title")}
      onClose={() => void requestClose()}
      closeOnScrim
      escHint
      initialFocus={() => promptBox}
      footer={
        <>
          <Button variant="ghost" disabled={busy() || confirmingClose()} onClick={() => void requestClose()}>
            {t("newTask.cancel")}
          </Button>
          <Button variant="primary" icon="plus" kbd={SUBMIT_KEYS} disabled={!canCreate()}
            loading={busy()} loadingLabel={t("newTask.creating")} onClick={() => void submit()}>
            {t("newTask.create")}
          </Button>
        </>
      }
    >
      <TextArea ref={promptBox} label={t("newTask.prompt")} value={prompt()}
        onInput={(event) => setPrompt(event.currentTarget.value)} />
      <div class="form-grid">
        <Input label={t("newTask.name")} value={title()} placeholder={titleFromPrompt(prompt())}
          onInput={(event) => setTitle(event.currentTarget.value)} />
        <Show when={props.isGit}>
          <Input mono label={t("newTask.base")} value={base()}
            placeholder={props.parents?.find((task) => task.id === parentTaskId())?.branch ?? props.defaultBranch}
            onInput={(event) => setBase(event.currentTarget.value)} />
        </Show>
      </div>
      <Show when={props.isGit && props.parents?.length}>
        <Select label={t("newTask.parent")} value={parentTaskId()}
          onChange={(event) => setParentTaskId(event.currentTarget.value)}>
          <option value="">{t("newTask.noParent")}</option>
          <For each={props.parents}>{(task) => <option value={task.id}>{task.title}</option>}</For>
        </Select>
      </Show>
      <fieldset class="form-field">
        <legend class="form-label">{t("newTask.agent")}</legend>
        <div class="agent-cards">
          <For each={AGENTS}>
            {(kind) => (
              <label class="agent-card" classList={{ "agent-card-on": agent() === kind }}>
                <input
                  type="radio"
                  name="agent"
                  checked={agent() === kind}
                  onChange={() => setAgent(kind)}
                />
                <b><AgentIcon agent={kind} />{AGENT_NAME[kind]}</b>
              </label>
            )}
          </For>
        </div>
      </fieldset>
      <Show when={error()}>{(message) => <p class="form-error">{message()}</p>}</Show>
    </Dialog>
  );
}
