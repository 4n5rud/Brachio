import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import { rememberPrompt } from "./prompt-history";

export type CueKind = "prompt" | "shell";

/** A reusable prompt or shell command of one project. */
export interface ProjectCue {
  id: string;
  repoId: string;
  kind: CueKind;
  label: string;
  body: string;
}

const [cuesByRepo, setCuesByRepo] = createSignal<ReadonlyMap<string, readonly ProjectCue[]>>(new Map());
const [managerRepo, setManagerRepo] = createSignal<string>();
const [shellRunner, setShellRunner] = createSignal<ShellCueRunner>();
let promptSender: PromptCueSender | undefined;

/** Types the prompt into the task's agent input without sending it; resolves to whether it went in. */
export type PromptCueSender = (taskId: string, text: string) => Promise<boolean>;
/** Runs the command in the task's shell once that shell is ready; resolves to whether it was run. */
export type ShellCueRunner = (taskId: string, command: string) => Promise<boolean>;

export const cuesOf = (repoId: string | undefined): readonly ProjectCue[] => (repoId ? cuesByRepo().get(repoId) ?? [] : []);
/** The project whose cues are being edited, if the manager is open. */
export const cueManagerRepo = managerRepo;
export const openCueManager = (repoId: string) => setManagerRepo(repoId);
export const closeCueManager = () => setManagerRepo(undefined);
/** Shell cues are offered only while an owner of the shell terminal has registered a runner. */
export const canRunShellCues = (): boolean => shellRunner() !== undefined;

const setFor = (repoId: string, cues: readonly ProjectCue[]) => setCuesByRepo((all) => new Map(all).set(repoId, cues));

export async function loadCues(repoId: string): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) return;
  setFor(repoId, await invoke<ProjectCue[]>("cue_list", { repoId }));
}

export async function addCue(repoId: string, kind: CueKind, label: string, body: string): Promise<void> {
  const cue = await invoke<ProjectCue>("cue_add", { repoId, kind, label, body });
  setFor(repoId, [...cuesOf(repoId), cue]);
}

export async function deleteCue(cue: ProjectCue): Promise<void> {
  await invoke("cue_delete", { id: cue.id });
  setFor(cue.repoId, cuesOf(cue.repoId).filter((item) => item.id !== cue.id));
}

/** The app registers how a prompt reaches the agent input. */
export function registerPromptCueSender(sender: PromptCueSender): () => void {
  promptSender = sender;
  return () => { if (promptSender === sender) promptSender = undefined; };
}

/** The owner of the shell terminal registers how a command is typed into a ready shell. */
export function registerShellCueRunner(runner: ShellCueRunner): () => void {
  setShellRunner(() => runner);
  return () => { if (shellRunner() === runner) setShellRunner(undefined); };
}

/** Runs one cue for a task. A prompt is typed without Enter; a shell command runs only after the shell is ready. */
export async function runCue(taskId: string, cue: ProjectCue): Promise<boolean> {
  if (cue.kind === "prompt") {
    const sent = (await promptSender?.(taskId, cue.body)) ?? false;
    if (sent) rememberPrompt(cue.repoId, taskId, "cue", cue.body);
    return sent;
  }
  return (await shellRunner()?.(taskId, cue.body)) ?? false;
}
