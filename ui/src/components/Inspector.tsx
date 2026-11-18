import { Match, Show, Switch, createSignal } from "solid-js";
import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { DiffStat } from "./DiffStat";

type Pane = "changes" | "qa" | "activity";

/** Right column: a compact summary of the selected task. */
export function Inspector(props: { task: TaskRow }) {
  const [pane, setPane] = createSignal<Pane>("changes");
  const tab = (id: Pane, label: string) => (
    <button type="button" aria-pressed={pane() === id} onClick={() => setPane(id)}>
      {label}
    </button>
  );
  return (
    <aside class="inspector">
      <div class="inspector-head">
        <div class="segmented" role="group">
          {tab("changes", t("inspector.changes"))}
          {tab("qa", t("inspector.qa"))}
          {tab("activity", t("inspector.activity"))}
        </div>
      </div>
      <Switch>
        <Match when={pane() === "changes"}>
          <section class="inspector-section">
            <h3>
              {t("inspector.changedFiles")}
              <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
            </h3>
          </section>
        </Match>
        <Match when={pane() === "qa"}>
          <section class="inspector-section">
            <Show when={props.task.qa} fallback={<p class="muted">{t("inspector.qa.none")}</p>}>
              {(qa) => (
                <div class="qa-summary">
                  <div>
                    <strong class="qa-passed">{qa().passed}</strong>
                    <span>{t("inspector.qa.passed")}</span>
                  </div>
                  <div>
                    <strong classList={{ "qa-failed": qa().failed > 0 }}>{qa().failed}</strong>
                    <span>{t("inspector.qa.failed")}</span>
                  </div>
                </div>
              )}
            </Show>
          </section>
        </Match>
        <Match when={pane() === "activity"}>
          <section class="inspector-section">
            <p class="muted">{t("inspector.activity.none")}</p>
          </section>
        </Match>
      </Switch>
    </aside>
  );
}
