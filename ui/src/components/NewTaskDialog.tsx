import { Button } from "./ui/Button";
import { For, Show, createEffect, createResource, createSignal, on, onCleanup, onMount } from "solid-js";
import { confirmDialog } from "./ui/ConfirmDialog";
import { Dialog } from "./ui/Dialog";
import { Input, Select, TextArea } from "./ui/Input";
import { Combobox } from "./ui/Combobox";
import { MenuSubmenu } from "./ui/Menu";
import { AddProjectItems } from "./AddProjectItems";
import { Icon } from "./Icon";
import { filterProjects, projectSubline, type AddProjectKind, type ProjectChoice } from "../lib/project-choices";
import { t } from "../i18n";
import { AGENT_CATALOG, AGENT_KINDS } from "../lib/agent-catalog";
import { AGENT_NAME, type AgentKind, type TaskRow } from "../lib/tasks";
import { profileLabel, profilesFor, repoProfiles } from "../lib/agent-profiles";
import { CORE_AGENT, detectAgents } from "../lib/terminal";
import { BranchField } from "./BranchField";
import { FolderAccessNotice } from "./FolderAccessNotice";
import { blockedFolder } from "../lib/folder-access";
import { AgentIcon } from "./AgentIcon";
import { Toggle } from "./ui/Toggle";
import { SUBMIT_KEYS } from "../lib/shortcut";
import { loadPromptHistory, recallStep, rememberPrompt, type PromptHistoryEntry } from "../lib/prompt-history";
import { AttachButton } from "./AttachButton";
import { fileReferences, isImagePath, MAX_ATTACHED_FILES, MAX_ATTACHED_IMAGES, relativeToProject, terminalExcerpt } from "../lib/attachments";
import { registerDialogDrop } from "../lib/dialog-drop";
import { readTerminalTail, terminalTailKeys } from "../lib/terminal-tail";
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
  /** Run the prompt once with Claude Code and once with Codex, to compare them. */
  compare?: boolean;
  /** A profile from the project's `brachio.toml`. */
  profile?: string;
}

const conversationLabel = (item: { id: string; timestamp: string | null }) => {
  const when = item.timestamp ? new Date(item.timestamp) : null;
  const label = when && !Number.isNaN(when.getTime()) ? when.toLocaleString() : "";
  return label ? `${label} · ${item.id.slice(0, 8)}` : item.id;
};

const AGENTS: readonly AgentKind[] = AGENT_KINDS;

/** The agent's own version line says "(Claude Code)" after the number; the card already shows the name. */
const plainVersion = (version: string | null) => version?.replace(/\s*\(Claude Code\)/i, "") ?? null;

const BAD_BASE = /invalid reference|not a valid (object name|commit)|unknown revision/i;

/** "New task" dialog: prompt, name, agent and base branch. */
export function NewTaskDialog(props: {
  repoId: string;
  repoName: string;
  /** Every project the task can go to; the field above the prompt switches between them. */
  projects: readonly ProjectChoice[];
  onProjectChange: (id: string) => void;
  /** The pinned "Add new project" submenu chose a way to bring one in. */
  onAddProject: (kind: AddProjectKind) => void;
  defaultBranch: string;
  /** A plain folder has no branch to base a task on and no parent task. */
  isGit: boolean;
  parents?: readonly TaskRow[];
  /** Names an open terminal, for the "last lines of a terminal" attachment. */
  terminalLabel?: (key: string) => string;
  onCreate: (input: NewTaskInput) => Promise<void>;
  onClose: () => void;
}) {
  const [prompt, setPrompt] = createSignal("");
  const [title, setTitle] = createSignal("");
  const [agent, setAgent] = createSignal<AgentKind>("claude");
  const [base, setBase] = createSignal("");
  const [projectText, setProjectText] = createSignal(props.repoName);
  const [parentTaskId, setParentTaskId] = createSignal("");
  const [conversationId, setConversationId] = createSignal("");
  const [compare, setCompare] = createSignal(false);
  const [profile, setProfile] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [history, setHistory] = createSignal<readonly PromptHistoryEntry[]>([]);
  let recallPosition = -1;
  let ownText = "";
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
  const [profiles] = createResource(() => props.repoId, (id) => repoProfiles(id).catch(() => []));
  const profileChoices = () => (compare() ? [] : profilesFor(profiles() ?? [], agent()));
  const conversationChoices = () => (agent() === "codex" && !compare() ? conversations() ?? [] : []);
  const agentInfo = (kind: AgentKind) => {
    const found = installed();
    if (!found) return { missing: false, version: null };
    const entry = found.find((item) => item.kind === CORE_AGENT[kind]);
    return { missing: !entry, version: entry?.version ?? null };
  };
  const canCompare = () => props.isGit && !parentTaskId() && !agentInfo("claude").missing && !agentInfo("codex").missing;
  const comparing = () => compare() && canCompare();
  const baseError = () => {
    const message = error();
    return message !== null && base().trim() !== "" && BAD_BASE.test(message)
      ? t("newTask.baseMissing").replace("{name}", () => base().trim()) : undefined;
  };
  const createError = () => (baseError() ? null : error());

  createEffect(() => setProjectText(props.repoName));
  createEffect(on(() => props.repoId, (repoId) => {
    recallPosition = -1;
    void loadPromptHistory(repoId).then((entries) => { if (repoId === props.repoId) setHistory(entries); }).catch(() => setHistory([]));
  }));
  const recall = (event: KeyboardEvent & { currentTarget: HTMLTextAreaElement }) => {
    if (event.isComposing || event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    const box = event.currentTarget;
    const atStart = box.selectionStart === 0 && box.selectionEnd === 0;
    const atEnd = box.selectionStart === box.value.length && box.selectionEnd === box.value.length;
    if (event.key === "ArrowUp" ? !atStart : !atEnd) return;
    const next = recallStep(recallPosition, history().length, event.key);
    if (next === undefined) return;
    event.preventDefault();
    if (recallPosition === -1) ownText = box.value;
    recallPosition = next;
    const text = next === -1 ? ownText : history()[next]?.body ?? "";
    setPrompt(text);
    box.value = text;
    const caret = event.key === "ArrowUp" ? 0 : text.length;
    box.setSelectionRange(caret, caret);
  };
  createEffect(on(() => props.repoId, () => {
    setBase("");
    setParentTaskId("");
    setConversationId("");
    setCompare(false);
    setError(null);
  }, { defer: true }));
  const projectOptions = () =>
    filterProjects(props.projects, projectText() === props.repoName ? "" : projectText()).map((project) => ({
      value: project.name,
      id: project.id,
      label: project.name,
      subline: projectSubline(project),
      icon: <Icon name={project.isGit ? "branch" : "folder"} class="icon-sm" />,
    }));

  const finalTitle = () => title().trim() || titleFromPrompt(prompt());
  const canCreate = () => !busy() && !confirmingClose() && finalTitle() !== "";
  const requestClose = async () => {
    if (busy() || confirmingClose()) return;
    if (!prompt().trim() && !title().trim() && !base().trim() && agent() === "claude" && !conversationId() && !compare()) {
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
      if (discard) {
        rememberPrompt(props.repoId, null, "draft", prompt());
        props.onClose();
      }
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
      rememberPrompt(props.repoId, null, "prompt", prompt());
      await props.onCreate({ prompt: prompt().trim(), title: finalTitle(), agent: agent(),
        base: base().trim(), parentTaskId: parentTaskId() || undefined,
        codexConversationId: agent() === "codex" && conversationId() && !comparing() ? conversationId() : undefined,
        compare: comparing() || undefined,
        profile: profileChoices().some((item) => item.name === profile()) ? profile() : undefined });
    } catch (failure) {
      setError(String(failure));
      setBusy(false);
    }
  };

  const addToPrompt = (text: string) =>
    setPrompt((current) => `${current}${current === "" || /\s$/u.test(current) ? "" : "\n"}${text}`);
  // The task's worktree does not exist yet, so images stay where they are and go in by their own path.
  const attachPaths = (paths: string[]) => {
    const root = props.projects.find((project) => project.id === props.repoId)?.path;
    const files = paths.filter((path) => !isImagePath(path));
    const images = paths.filter(isImagePath);
    if (files.length > MAX_ATTACHED_FILES || images.length > MAX_ATTACHED_IMAGES) {
      setError(t("attach.tooMany").replace("{files}", String(MAX_ATTACHED_FILES)).replace("{images}", String(MAX_ATTACHED_IMAGES)));
      return;
    }
    addToPrompt(fileReferences([...files.map((path) => (root ? relativeToProject(root, path) : path)), ...images]) + " ");
  };
  onCleanup(registerDialogDrop(attachPaths));

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
          <Button variant="primary" icon="plus" kbd={SUBMIT_KEYS()} disabled={!canCreate()}
            loading={busy()} loadingLabel={t("newTask.creating")} onClick={() => void submit()}>
            {createError() ? t("newTask.retry") : t("newTask.create")}
          </Button>
        </>
      }
    >
      <TextArea ref={promptBox} label={t("newTask.prompt")} value={prompt()}
        onInput={(event) => { recallPosition = -1; setPrompt(event.currentTarget.value); }}
        onKeyDown={recall} />
      <div class="attach-row">
        <AttachButton defaultPath={props.projects.find((project) => project.id === props.repoId)?.path}
          tails={() => terminalTailKeys().map((key) => ({ key, label: props.terminalLabel?.(key) ?? key }))}
          onPaths={(paths) => attachPaths(paths)}
          onTail={(key) => {
            const lines = readTerminalTail(key);
            if (lines) addToPrompt(terminalExcerpt(props.terminalLabel?.(key) ?? key, lines));
          }} />
      </div>
      <div class="form-grid">
        <Combobox label={t("project.label")} listLabel={t("project.list")} value={projectText()}
          options={projectOptions()} emptyText={t("project.empty")}
          onValue={setProjectText}
          onPick={(option) => { if (option.id) props.onProjectChange(option.id); }}
          onBlur={() => setProjectText(props.repoName)}
          footer={(api) => (
            <MenuSubmenu icon={<Icon name="plus" class="icon-sm" />} label={t("project.addNew")}>
              <AddProjectItems onPick={(kind) => { api.close(); props.onAddProject(kind); }} />
            </MenuSubmenu>
          )} />
        <Input label={t("newTask.name")} value={title()} placeholder={titleFromPrompt(prompt())}
          onInput={(event) => setTitle(event.currentTarget.value)} />
      </div>
      <Show when={props.isGit}>
        <BranchField repoId={props.repoId} value={base()}
          placeholder={props.parents?.find((task) => task.id === parentTaskId())?.branch ?? props.defaultBranch}
          error={baseError()}
          onValue={(value) => { setBase(value); setError(null); }} />
      </Show>
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
                  disabled={agentInfo(kind).missing || comparing()}
                  onChange={() => { setAgent(kind); setConversationId(""); setProfile(""); }}
                />
                <AgentIcon agent={kind} />
                <span class="agent-card-text">
                  <b>{AGENT_NAME[kind]}</b>
                  <Show when={agentInfo(kind).missing || agentInfo(kind).version}>
                    <span class="agent-card-note">
                      {agentInfo(kind).missing
                        ? t("newTask.agentMissing").replace("{name}", () => kind) : plainVersion(agentInfo(kind).version)}
                    </span>
                  </Show>
                </span>
              </label>
            )}
          </For>
        </div>
        <Show when={!AGENT_CATALOG[agent()].approvalRelay}>
          <p class="form-hint">{t("newTask.agentOwnTerminal").replace("{name}", () => AGENT_NAME[agent()])}</p>
        </Show>
      </fieldset>
      <Show when={profileChoices().length > 0}>
        <Select label={t("newTask.profile")} hint={t("newTask.profileHint")} value={profile()}
          onChange={(event) => setProfile(event.currentTarget.value)}>
          <option value="">{t("newTask.profileNone")}</option>
          <For each={profileChoices()}>{(item) => <option value={item.name}>{profileLabel(item)}</option>}</For>
        </Select>
      </Show>
      <Show when={canCompare()}>
        <Toggle checked={compare()} onChange={setCompare} label={t("newTask.compare")} hint={t("newTask.compareHint")} />
      </Show>
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
