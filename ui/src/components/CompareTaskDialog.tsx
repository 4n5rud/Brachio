import { For, Show, createResource, createSignal } from "solid-js";
import { t } from "../i18n";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Notice } from "./ui/Notice";
import { AGENT_NAME } from "../lib/tasks";
import { compareChoices, repoProfiles, type CompareTarget } from "../lib/agent-profiles";

const same = (a: CompareTarget, b: CompareTarget) => a.agent === b.agent && a.profile === b.profile;

/** Picks the agents and profiles to run the same prompt with, each in its own worktree. */
export function CompareTaskDialog(props: {
  title: string;
  repoId: string;
  onCreate: (targets: CompareTarget[]) => Promise<void>;
  onClose: () => void;
}) {
  const [profiles] = createResource(() => props.repoId, (id) => repoProfiles(id).catch(() => []));
  const [chosen, setChosen] = createSignal<CompareTarget[]>([]);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const toggle = (target: CompareTarget) =>
    setChosen((list) => (list.some((item) => same(item, target)) ? list.filter((item) => !same(item, target)) : [...list, target]));
  const submit = async () => {
    setBusy(true);
    setError(undefined);
    try {
      await props.onCreate(chosen());
      props.onClose();
    } catch (failure) {
      setError(String(failure));
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("compareTask.title")} size="confirm" onClose={props.onClose}
      footer={<>
        <Button variant="ghost" onClick={props.onClose}>{t("confirm.cancel")}</Button>
        <Button variant="primary" disabled={busy() || chosen().length === 0} onClick={() => void submit()}>
          {t("compareTask.create")}
        </Button>
      </>}>
      <p class="muted">{t("compareTask.note").replace("{title}", () => props.title)}</p>
      <fieldset class="form-field">
        <legend class="form-label">{t("compareTask.runWith")}</legend>
        <For each={compareChoices(profiles() ?? [])}>
          {(target) => (
            <label class="compare-choice" style={{ display: "block", padding: "4px 0" }}>
              <input type="checkbox" checked={chosen().some((item) => same(item, target))} onChange={() => toggle(target)} />
              {" "}{AGENT_NAME[target.agent]}{target.profile ? ` · ${target.profile}` : ""}
            </label>
          )}
        </For>
      </fieldset>
      <Show when={error()}>{(message) => <Notice tone="error" lead={t("error.title")}>{message()}</Notice>}</Show>
    </Dialog>
  );
}
