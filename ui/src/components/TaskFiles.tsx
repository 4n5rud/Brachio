import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { t } from "../i18n";
import { listTaskFiles, previewTaskFile } from "../lib/workspace";

const RESULT_LIMIT = 100;

export function TaskFiles(props: { taskId: string }) {
  const [filter, setFilter] = createSignal("");
  const [path, setPath] = createSignal("");
  const [files, { refetch }] = createResource(async () => {
    try {
      return { paths: await listTaskFiles(props.taskId) };
    } catch (failure) {
      return { error: String(failure) };
    }
  });
  const [preview] = createResource(path, async (selected) => {
    if (!selected) return undefined;
    try {
      return { path: selected, file: await previewTaskFile(props.taskId, selected) };
    } catch (failure) {
      return { path: selected, error: String(failure) };
    }
  });
  const matches = createMemo(() => {
    const query = filter().trim().toLowerCase();
    return (files()?.paths ?? []).filter((file) => file.toLowerCase().includes(query));
  });

  return (
    <>
      <label class="form-field">
        <span>{t("files.filter")}</span>
        <input class="input" value={filter()} onInput={(event) => setFilter(event.currentTarget.value)} />
      </label>
      <button class="button button-sm" type="button" disabled={files.loading} onClick={() => void refetch()}>
        {t("inspector.refresh")}
      </button>
      <Show when={files()?.error}>{(error) => <p role="alert">{error()}</p>}</Show>
      <Show when={!files.loading && !files()?.error && matches().length === 0}>
        <p class="muted">{t("files.empty")}</p>
      </Show>
      <div class="quick-open-results">
        <For each={matches().slice(0, RESULT_LIMIT)}>{(file) => (
          <button class="quick-open-result" classList={{ selected: file === path() }} type="button"
            aria-pressed={file === path()} onClick={() => setPath(file)}>{file}</button>
        )}</For>
      </div>
      <Show when={matches().length > RESULT_LIMIT}><p class="muted">{t("files.more")}</p></Show>
      <Show when={preview.loading}><p role="status">{t("files.loading")}</p></Show>
      <Show when={!preview.loading && preview()?.path === path() && preview()}>
        {(result) => (
          <>
            <Show when={result().error}>{(error) => <p role="alert">{error()}</p>}</Show>
            <Show when={result().file}>{(file) => (
              <>
                <Show when={file().text !== null} fallback={<p class="muted">{t("files.binary")}</p>}>
                  <pre class="inspector-patch" tabIndex={0} aria-label={path()}>{file().text}</pre>
                </Show>
                <Show when={file().truncated}><p class="muted">{t("files.truncated")}</p></Show>
              </>
            )}</Show>
          </>
        )}
      </Show>
    </>
  );
}
