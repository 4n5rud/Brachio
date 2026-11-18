import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import { AGENT_NAME, type AgentKind } from "../lib/tasks";
import { SUBMIT_KEYS } from "../lib/shortcut";
import { titleFromPrompt } from "../lib/workspace";
import { Icon } from "./Icon";

/** What the dialog hands back when the user creates a task. */
export interface NewTaskInput {
  prompt: string;
  title: string;
  agent: AgentKind;
  /** Empty means the repository's default branch. */
  base: string;
}

const AGENTS: readonly AgentKind[] = ["claude", "codex"];

/** "New task" dialog: prompt, name, agent and base branch. */
export function NewTaskDialog(props: {
  defaultBranch: string;
  onCreate: (input: NewTaskInput) => Promise<void>;
  onClose: () => void;
}) {
  const [prompt, setPrompt] = createSignal("");
  const [title, setTitle] = createSignal("");
  const [agent, setAgent] = createSignal<AgentKind>("claude");
  const [base, setBase] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const finalTitle = () => title().trim() || titleFromPrompt(prompt());
  const canCreate = () => !busy() && finalTitle() !== "";

  const submit = async () => {
    if (!canCreate()) return;
    setBusy(true);
    setError(null);
    try {
      await props.onCreate({ prompt: prompt().trim(), title: finalTitle(), agent: agent(), base: base().trim() });
    } catch (failure) {
      setError(String(failure));
      setBusy(false);
    }
  };

  let promptBox!: HTMLTextAreaElement;
  onMount(() => promptBox.focus());
  const onKey = (event: KeyboardEvent) => {
    if (event.key === "Escape") props.onClose();
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) void submit();
  };
  window.addEventListener("keydown", onKey);
  onCleanup(() => window.removeEventListener("keydown", onKey));

  return (
    <div class="scrim" onClick={(event) => event.target === event.currentTarget && props.onClose()}>
      <div class="modal" role="dialog" aria-modal="true" aria-labelledby="new-task-title">
        <div class="modal-head">
          <h2 id="new-task-title">{t("newTask.title")}</h2>
          <kbd>Esc</kbd>
        </div>
        <div class="modal-body">
          <label class="form-field">
            <span class="form-label">{t("newTask.prompt")}</span>
            <textarea
              ref={promptBox}
              class="input input-area"
              value={prompt()}
              onInput={(event) => setPrompt(event.currentTarget.value)}
            />
          </label>
          <div class="form-grid">
            <label class="form-field">
              <span class="form-label">{t("newTask.name")}</span>
              <input
                class="input"
                value={title()}
                placeholder={titleFromPrompt(prompt())}
                onInput={(event) => setTitle(event.currentTarget.value)}
              />
            </label>
            <label class="form-field">
              <span class="form-label">{t("newTask.base")}</span>
              <input
                class="input input-mono"
                value={base()}
                placeholder={props.defaultBranch}
                onInput={(event) => setBase(event.currentTarget.value)}
              />
            </label>
          </div>
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
                    <b>{AGENT_NAME[kind]}</b>
                  </label>
                )}
              </For>
            </div>
          </fieldset>
          <Show when={error()}>{(message) => <p class="form-error">{message()}</p>}</Show>
        </div>
        <div class="modal-foot">
          <button class="button button-ghost" type="button" onClick={() => props.onClose()}>
            {t("newTask.cancel")}
          </button>
          <button class="button button-primary" type="button" disabled={!canCreate()} onClick={() => void submit()}>
            <Icon name="plus" />
            {busy() ? t("newTask.creating") : t("newTask.create")}
            <kbd>{SUBMIT_KEYS}</kbd>
          </button>
        </div>
      </div>
    </div>
  );
}
