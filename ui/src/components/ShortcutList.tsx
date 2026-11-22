import { For, Show, createSignal, onCleanup } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { SHORTCUTS, SHORTCUT_GROUPS, shortcutKeys, type ShortcutGroup } from "../lib/shortcut";
import {
  REBINDABLE, bindShortcut, chordFromEvent, isCustomized, resetAllShortcuts, resetShortcut,
  type ChordProblem, type RebindableId,
} from "../lib/shortcut-bindings";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Kbd } from "./ui/Kbd";
import "./ShortcutList.css";

type ShortcutId = keyof typeof SHORTCUTS;

const GROUP_LABEL: Record<ShortcutGroup, MessageKey> = {
  global: "shortcuts.group.global",
  task: "shortcuts.group.task",
  terminal: "shortcuts.group.terminal",
  palette: "shortcuts.group.palette",
};

const ids = Object.keys(SHORTCUTS) as ShortcutId[];
const isRebindable = (id: string): id is RebindableId => (REBINDABLE as readonly string[]).includes(id);

function problemText(problem: ChordProblem): string {
  switch (problem.reason) {
    case "noModifier": return t("shortcuts.problem.noModifier");
    case "terminal": return t("shortcuts.problem.terminal");
    case "fixed":
    case "taken": {
      const owner = SHORTCUTS[problem.owner as ShortcutId];
      return t("shortcuts.problem.taken").replace("{action}", () => (owner ? t(owner.label) : problem.owner));
    }
  }
}

/** Every shortcut of this platform, grouped by where it applies; with `editable`, each can be rebound. */
export function ShortcutList(props: { editable?: boolean }) {
  const [recording, setRecording] = createSignal<RebindableId>();
  const [problem, setProblem] = createSignal<ChordProblem>();
  const anyCustom = () => REBINDABLE.some((id) => isCustomized(id));

  const stopRecording = () => {
    window.removeEventListener("keydown", capture, true);
    setRecording(undefined);
  };
  function capture(event: KeyboardEvent) {
    const id = recording();
    if (!id) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      setProblem(undefined);
      stopRecording();
      return;
    }
    const chord = chordFromEvent(event);
    if (!chord) return;
    const failed = bindShortcut(id, chord);
    setProblem(failed);
    if (!failed) stopRecording();
  }
  onCleanup(() => window.removeEventListener("keydown", capture, true));
  const record = (id: RebindableId) => {
    setProblem(undefined);
    setRecording(id);
    window.addEventListener("keydown", capture, true);
  };

  return (
    <div class="shortcut-list">
      <For each={SHORTCUT_GROUPS}>
        {(group) => (
          <section class="shortcut-group">
            <h3 class="shortcut-group-title">{t(GROUP_LABEL[group])}</h3>
            <For each={ids.filter((id) => SHORTCUTS[id].group === group)}>
              {(id) => (
                <div class="shortcut-row" classList={{ "shortcut-row-editable": props.editable }}>
                  <span>{t(SHORTCUTS[id].label)}</span>
                  <Show when={recording() === id} fallback={<Kbd>{shortcutKeys(id)}</Kbd>}>
                    <span class="shortcut-recording" role="status">{t("shortcuts.pressKeys")}</span>
                  </Show>
                  <Show when={props.editable}>
                    <span class="shortcut-actions">
                      <Show when={isRebindable(id)}>
                        <Button variant="ghost" size="sm" onClick={() => (recording() === id ? stopRecording() : record(id as RebindableId))}>
                          {recording() === id ? t("confirm.cancel") : t("shortcuts.change")}
                        </Button>
                        <Show when={isCustomized(id as RebindableId)}>
                          <Button variant="ghost" size="sm" onClick={() => { resetShortcut(id as RebindableId); setProblem(undefined); }}>
                            {t("shortcuts.reset")}
                          </Button>
                        </Show>
                      </Show>
                    </span>
                  </Show>
                  <Show when={recording() === id && problem()}>
                    {(current) => <p class="form-error shortcut-problem" role="alert">{problemText(current())}</p>}
                  </Show>
                </div>
              )}
            </For>
          </section>
        )}
      </For>
      <Show when={props.editable && anyCustom()}>
        <div>
          <Button size="sm" onClick={() => { resetAllShortcuts(); setProblem(undefined); }}>{t("shortcuts.resetAll")}</Button>
        </div>
      </Show>
    </div>
  );
}

/** Overlay with the shortcuts of the platform in use, grouped by where they apply. */
export function ShortcutsDialog(props: { onClose: () => void }) {
  return (
    <Dialog title={t("shortcuts.title")} onClose={props.onClose} closeOnScrim escHint>
      <ShortcutList />
    </Dialog>
  );
}
