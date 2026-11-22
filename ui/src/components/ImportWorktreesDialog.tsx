import { For, Show, createResource, createSignal } from "solid-js";
import { t } from "../i18n";
import { importWorktrees, listImportableWorktrees, type TaskRecord } from "../lib/workspace";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { PathLink } from "./ui/PathLink";
import "./ImportWorktreesDialog.css";

/** Lists the project's Git worktrees that no task uses and turns the chosen ones into tasks. */
export function ImportWorktreesDialog(props: {
  repoId: string;
  repoName: string;
  onImported: (records: TaskRecord[]) => void;
  onClose: () => void;
}) {
  const [candidates] = createResource(async () => {
    try {
      return { items: await listImportableWorktrees(props.repoId) };
    } catch (failure) {
      return { items: [], error: String(failure) };
    }
  });
  const [chosen, setChosen] = createSignal<ReadonlySet<string>>(new Set());
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const toggle = (path: string) =>
    setChosen((current) => {
      const next = new Set(current);
      if (!next.delete(path)) next.add(path);
      return next;
    });
  const submit = async () => {
    if (busy() || chosen().size === 0) return;
    setBusy(true);
    setError(undefined);
    try {
      props.onImported(await importWorktrees(props.repoId, [...chosen()]));
      props.onClose();
    } catch (failure) {
      setError(String(failure));
      setBusy(false);
    }
  };
  const items = () => candidates()?.items ?? [];
  const importLabel = () => (chosen().size === 1 ? t("import.one") : t("import.many")).replace("{count}", String(chosen().size));
  return (
    <Dialog
      title={<>{t("import.title")}<span class="modal-sub">{props.repoName}</span></>}
      onClose={props.onClose}
      closeOnScrim
      escHint
      footer={
        <>
          <Button variant="ghost" disabled={busy()} onClick={props.onClose}>{t("newTask.cancel")}</Button>
          <Button variant="primary" disabled={busy() || chosen().size === 0} loading={busy()}
            loadingLabel={t("import.importing")} onClick={() => void submit()}>
            {chosen().size === 0 ? t("import.action") : importLabel()}
          </Button>
        </>
      }
    >
      <Show when={!candidates.loading} fallback={<p class="form-hint">{t("import.loading")}</p>}>
        <Show when={!candidates()?.error} fallback={<p class="form-hint" role="alert">{candidates()?.error}</p>}>
          <Show when={items().length > 0} fallback={<p class="form-hint">{t("import.none")}</p>}>
            <p class="form-hint">{(items().length === 1 ? t("import.foundOne") : t("import.foundMany")).replace("{count}", String(items().length)).replace("{name}", props.repoName)}</p>
            <For each={items()}>
              {(item) => (
                <label class="row-gap import-row" title={item.path}>
                  <input type="checkbox" checked={chosen().has(item.path)} onChange={() => toggle(item.path)} />
                  <PathLink path={item.path} copyable={false} />
                  <span class="import-branch">{item.branch}</span>
                </label>
              )}
            </For>
          </Show>
        </Show>
      </Show>
      <Show when={error()}>{(message) => <p class="form-hint" role="alert">{message()}</p>}</Show>
      <p class="form-hint">{t("import.note")}</p>
    </Dialog>
  );
}
