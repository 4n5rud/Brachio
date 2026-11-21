import { Button } from "./ui/Button";
import { For, Show, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { confirmDialog } from "./ui/ConfirmDialog";
import { Dialog } from "./ui/Dialog";
import { Input, Select, TextArea } from "./ui/Input";
import { t } from "../i18n";
import { AGENT_NAME, type AgentKind, type TaskRow } from "../lib/tasks";
import { CORE_AGENT, detectAgents } from "../lib/terminal";
import { BranchField } from "./BranchField";
import { FolderAccessNotice } from "./FolderAccessNotice";
import { blockedFolder } from "../lib/folder-access";
import { AgentIcon } from "./AgentIcon";
import { SUBMIT_KEYS } from "../lib/shortcut";
import { repoCodexConversations, titleFromPrompt } from "../lib/workspace";

/** What the dialog hands back when the user creates a task. */
export interface NewTaskInput {
  prompt: string;
  title: string;
  agent: AgentKind;
  /** Empty means the repository's default branch. */
  base: string;
  parentTaskId?: string;
  /** A Codex conversation recorded for the project that the task continues. */
  codexConversationId?: string;
}

const conversationLabel = (item: { id: string; timestamp: string | null }) => {
  const when = item.timestamp ? new Date(item.timestamp) : null;
  const label = when && !Number.isNaN(when.getTime()) ? when.toLocaleString() : "";
  return label ? `${label} · ${item.id.slice(0, 8)}` : item.id;
};

const AGENTS: readonly AgentKind[] = ["claude", "codex"];

const BAD_BASE = /invalid reference|not a valid (object name|commit)|unknown revision/i;

/** "New task" dialog: prompt, name, agent and base branch. */
export function NewTaskDialog(props: {
  repoId: string;
  repoName: string;
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
  const [conversationId, setConversationId] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [confirmingClose, setConfirmingClose] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [installed] = createResource(async () => {
    try {
      return await detectAgents();
    } catch {
      return undefined;
    }
  });
  const [conversations] = createResource(
    () => (agent() === "codex" ? props.repoId : false),
    async (repoId) => {
      try {
        return await repoCodexConversations(repoId);
      } catch {
        return [];
      }
    },
  );
  const conversationChoices = () => (agent() === "codex" ? conversations() ?? [] : []);
  const agentInfo = (kind: AgentKind) => {
    const found = installed();
    if (!found) return { missing: false, version: null };
    const entry = found.find((item) => item.kind === CORE_AGENT[kind]);
    return { missing: !entry, version: entry?.version ?? null };
  };
  const baseError = () => {
    const message = error();
    return message !== null && base().trim() !== "" && BAD_BASE.test(message)
      ? t("newTask.baseMissing").replace("{name}", () => base().trim()) : undefined;
  };
  const createError = () => (baseError() ? null : error());

  const finalTitle = () => title().trim() || titleFromPrompt(prompt());
  const canCreate = () => !busy() && !confirmingClose() && finalTitle() !== "";
  const requestClose = async () => {
    if (busy() || confirmingClose()) return;
    if (!prompt().trim() && !title().trim() && !base().trim() && agent() === "claude" && !conversationId()) {
      props.onClose();
      return;
    }
    setConfirmingClose(true);
    try {
      const discard = await confirmDialog({
        title: t("newTask.discardDraft"),
        body: t("newTask.discardBody"),
        cancelLabel: t("newTask.keepEditing"),
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
        base: base().trim(), parentTaskId: parentTaskId() || undefined,
        codexConversationId: agent() === "codex" && conversationId() ? conversationId() : undefined });
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
      title={<>{t("newTask.title")}<span class="modal-sub">{props.repoName}</span></>}
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
            {createError() ? t("newTask.retry") : t("newTask.create")}
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
          <BranchField repoId={props.repoId} value={base()}
            placeholder={props.parents?.find((task) => task.id === parentTaskId())?.branch ?? props.defaultBranch}
            error={baseError()}
            onValue={(value) => { setBase(value); setError(null); }} />
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
              <label class="agent-card" classList={{ "agent-card-on": agent() === kind, "agent-card-off": agentInfo(kind).missing }}>
                <input
                  type="radio"
                  name="agent"
                  checked={agent() === kind}
                  disabled={agentInfo(kind).missing}
                  onChange={() => { setAgent(kind); setConversationId(""); }}
                />
                <b><AgentIcon agent={kind} />{AGENT_NAME[kind]}</b>
                <Show when={agentInfo(kind).missing || agentInfo(kind).version}>
                  <span class="agent-card-note">
                    {agentInfo(kind).missing
                      ? t("newTask.agentMissing").replace("{name}", () => kind) : agentInfo(kind).version}
                  </span>
                </Show>
              </label>
            )}
          </For>
        </div>
      </fieldset>
      <Show when={conversationChoices().length > 0}>
        <Select label={t("newTask.codexConversation")} hint={t("newTask.codexConversationHint")}
          value={conversationId()} onChange={(event) => setConversationId(event.currentTarget.value)}>
          <option value="">{t("newTask.codexNewConversation")}</option>
          <For each={conversationChoices()}>
            {(item) => <option value={item.id}>{conversationLabel(item)}</option>}
          </For>
        </Select>
      </Show>
      <Show when={createError() && blockedFolder(createError() ?? "")}>
        {(folder) => <FolderAccessNotice path={folder()} onRetry={() => void submit()} />}
      </Show>
      <Show when={createError() && blockedFolder(createError() ?? "") === null ? createError() : null}>
        {(message) => (
          <div class="create-failed" role="alert">
            <strong>{t("newTask.createFailed")}</strong>
            <pre class="create-log">{message()}</pre>
            <p class="form-hint">{t("newTask.createFailedNote")}</p>
          </div>
        )}
      </Show>
    </Dialog>
  );
}
