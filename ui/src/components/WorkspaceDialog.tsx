import { For, Match, Show, Switch, createMemo, createSignal, onMount } from "solid-js";
import { locale, setLocale, t } from "../i18n";
import { NEW_TASK_KEYS, QUICK_OPEN_KEYS } from "../lib/shortcut";
import type { TaskRow } from "../lib/tasks";

export type Theme = "system" | "light" | "dark";

export function WorkspaceDialog(props: {
  panel: "quickOpen" | "settings";
  tasks: readonly TaskRow[];
  theme: Theme;
  notifications: boolean;
  onTheme: (theme: Theme) => void;
  onNotifications: (enabled: boolean) => void;
  onOpen: (id: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = createSignal("");
  const [highlight, setHighlight] = createSignal(0);
  const matches = createMemo(() => {
    const terms = query().trim().toLowerCase().split(/\s+/);
    return props.tasks.filter((task) => {
      const searchable = `${task.title} ${task.branch}`.toLowerCase();
      return terms.every((term) => searchable.includes(term));
    });
  });
  let dialog!: HTMLDialogElement;
  onMount(() => dialog.showModal());

  const choose = (id: string) => {
    props.onOpen(id);
    dialog.close();
  };
  return (
    <dialog ref={dialog} class="modal workspace-dialog" aria-labelledby="workspace-dialog-title"
      onClose={props.onClose}>
      <div class="modal-head">
        <h2 id="workspace-dialog-title">
          {t(props.panel === "quickOpen" ? "quickOpen.title" : "titlebar.settings")}
        </h2>
        <button class="button button-sm" type="button" onClick={() => dialog.close()}>{t("workspace.close")}</button>
      </div>
      <div class="modal-body">
        <Switch>
          <Match when={props.panel === "quickOpen"}>
            <input autofocus class="input" aria-label={t("titlebar.search")}
              placeholder={t("quickOpen.placeholder")} value={query()}
              onInput={(event) => { setQuery(event.currentTarget.value); setHighlight(0); }}
              onKeyDown={(event) => {
                if (event.isComposing) return;
                const count = matches().length;
                if (!count) return;
                if (event.key === "Enter") {
                  event.preventDefault();
                  const task = matches()[highlight()] ?? matches()[0];
                  if (task) choose(task.id);
                } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
                  event.preventDefault();
                  const step = event.key === "ArrowDown" ? 1 : -1;
                  const next = (highlight() + count + step) % count;
                  setHighlight(next);
                  dialog.querySelectorAll(".quick-open-result")[next]?.scrollIntoView({ block: "nearest" });
                }
              }} />
            <Show when={matches().length > 0} fallback={<p class="muted">{t("quickOpen.empty")}</p>}>
              <div class="quick-open-results">
                <For each={matches()}>{(task, index) => (
                  <button class="quick-open-result" classList={{ selected: highlight() === index() }}
                    type="button" onClick={() => choose(task.id)}>
                    <strong>{task.title}</strong>
                    <span>{task.branch}</span>
                  </button>
                )}</For>
              </div>
            </Show>
          </Match>
          <Match when={props.panel === "settings"}>
            <label class="form-field">
              <span class="form-label">{t("settings.theme")}</span>
              <select class="input" value={props.theme} onChange={(event) => {
                const theme = event.currentTarget.value;
                if (theme === "system" || theme === "light" || theme === "dark") props.onTheme(theme);
              }}>
                <option value="system">{t("settings.system")}</option>
                <option value="light">{t("settings.light")}</option>
                <option value="dark">{t("settings.dark")}</option>
              </select>
            </label>
            <label class="form-field">
              <span class="form-label">{t("settings.language")}</span>
              <select class="input" value={locale()} onChange={(event) =>
                setLocale(event.currentTarget.value === "ko" ? "ko" : "en")}>
                <option value="en">English</option>
                <option value="ko">한국어</option>
              </select>
            </label>
            <label>
              <input type="checkbox" checked={props.notifications}
                onChange={(event) => props.onNotifications(event.currentTarget.checked)} />
              {" "}{t("settings.notifications")}
            </label>
            <div class="workspace-shortcuts">
              <span>{t("quickOpen.title")}</span><kbd>{QUICK_OPEN_KEYS}</kbd>
              <span>{t("newTask.title")}</span><kbd>{NEW_TASK_KEYS}</kbd>
            </div>
          </Match>
        </Switch>
      </div>
    </dialog>
  );
}
