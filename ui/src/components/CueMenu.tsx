import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { canRunShellCues, cuesOf, openCueManager, runCue } from "../lib/cues";
import { pushToast } from "../lib/toasts";
import { IconButton } from "./ui/IconButton";
import { Menu, MenuItem, MenuSeparator } from "./ui/Menu";

/** The project's cues in the task header: run one on this task, or open the manager. */
export function CueMenu(props: { taskId: string; repoId: string | undefined }) {
  const [anchor, setAnchor] = createSignal<HTMLElement>();
  const run = (cue: Parameters<typeof runCue>[1]) => {
    setAnchor(undefined);
    void runCue(props.taskId, cue).then((done) => {
      if (!done) pushToast({ tone: "error", message: t("cue.notRun") });
    });
  };
  return (
    <Show when={props.repoId}>{(repoId) => (
      <>
        <IconButton icon="list" label={t("cue.title")} onClick={(event) => setAnchor(event.currentTarget)} />
        <Show when={anchor()}>{(element) => (
          <Menu anchor={element()} label={t("cue.title")} onClose={() => setAnchor(undefined)}>
            <For each={cuesOf(repoId())}>{(cue) => (
              <MenuItem disabled={cue.kind === "shell" && !canRunShellCues()} hint={cue.kind === "shell" ? t("cue.shell") : undefined}
                onSelect={() => run(cue)}>{cue.label}</MenuItem>
            )}</For>
            <Show when={cuesOf(repoId()).length > 0}><MenuSeparator /></Show>
            <MenuItem onSelect={() => { setAnchor(undefined); openCueManager(repoId()); }}>{t("cue.manage")}</MenuItem>
          </Menu>
        )}</Show>
      </>
    )}</Show>
  );
}
