import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { previewTaskMerge, type TaskMergePreview as MergePreview } from "../lib/workspace";

export function TaskMergePreview(props: { taskId: string }) {
  const [result, setResult] = createSignal<MergePreview>();
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  const check = async () => {
    setBusy(true);
    setError("");
    setResult(undefined);
    try {
      setResult(await previewTaskMerge(props.taskId));
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="merge-preview">
      <button class="button button-sm" type="button" disabled={busy()} onClick={() => void check()}>
        {t(busy() ? "mergePreview.checking" : "mergePreview.check")}
      </button>
      <Show when={result()} keyed>
        {(preview) => (
          <div classList={{ "worktree-conflicts": preview.conflicts.length > 0 }} role="status">
            <p>{t(preview.conflicts.length ? "mergePreview.conflicts" : "mergePreview.clean")}{" "}
              <code>{preview.baseBranch}</code>
            </p>
            <Show when={preview.conflicts.length > 0}>
              <ul><For each={preview.conflicts}>{(path) => <li><code>{path}</code></li>}</For></ul>
            </Show>
          </div>
        )}
      </Show>
      <Show when={error()}><p role="alert">{error()}</p></Show>
    </div>
  );
}
