import { Input } from "./ui/Input";
import { Button } from "./ui/Button";
import { For, Show, createEffect, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { listTaskFiles, previewTaskFile, previewTaskMedia } from "../lib/workspace";

const RESULT_LIMIT = 100;
const mediaTypes = new Map([
  ["png", "image/png"], ["jpg", "image/jpeg"], ["jpeg", "image/jpeg"],
  ["gif", "image/gif"], ["webp", "image/webp"], ["pdf", "application/pdf"],
]);

export function TaskFiles(props: { taskId: string }) {
  const [filter, setFilter] = createSignal("");
  const [path, setPath] = createSignal("");
  const [media, setMedia] = createSignal<{ url: string; type: string }>();
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
      const type = mediaTypes.get(selected.split(".").pop()?.toLowerCase() ?? "");
      if (type) {
        const response = await previewTaskMedia(props.taskId, selected);
        const bytes = Array.isArray(response) ? Uint8Array.from(response) : new Uint8Array(response);
        return { path: selected, blob: new Blob([bytes], { type }) };
      }
      return { path: selected, file: await previewTaskFile(props.taskId, selected) };
    } catch (failure) {
      return { path: selected, error: String(failure) };
    }
  });
  createEffect(() => {
    const result = preview();
    if (preview.loading || result?.path !== path() || !result.blob) {
      setMedia(undefined);
      return;
    }
    const url = URL.createObjectURL(result.blob);
    setMedia({ url, type: result.blob.type });
    onCleanup(() => URL.revokeObjectURL(url));
  });
  const matches = createMemo(() => {
    const query = filter().trim().toLowerCase();
    return (files()?.paths ?? []).filter((file) => file.toLowerCase().includes(query));
  });

  return (
    <>
      <Input label={t("files.filter")} value={filter()} onInput={(event) => setFilter(event.currentTarget.value)} />
      <Button size="sm" disabled={files.loading} onClick={() => void refetch()}>
        {t("inspector.refresh")}
      </Button>
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
      <Show when={media()} keyed>
        {(item) => item.type === "application/pdf"
          ? <iframe class="file-preview-pdf" src={item.url} title={path()} />
          : <img class="file-preview-image" src={item.url} alt={path()} />}
      </Show>
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
