import { Button } from "./ui/Button";
import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { previewTaskMerge, type TaskMergePreview as MergePreview } from "../lib/workspace";
import { FileRow } from "./FileRow";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";

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

  return <MergeCheckView result={result()} error={error()} busy={busy()} onCheck={() => void check()} />;
}

export function MergeCheckView(props: { result?: MergePreview; error?: string; busy: boolean; onCheck: () => void }) {
  return (
    <InspectorSection title={t("mergePreview.title")}>
      <Show when={props.result} keyed>
        {(preview) => (
          <>
            <InspectorNote tone={preview.conflicts.length ? "error" : "ok"}>
              {t(preview.conflicts.length ? "mergePreview.conflicts" : "mergePreview.clean")}{" "}
              <code>{preview.baseBranch}</code>
            </InspectorNote>
            <For each={preview.conflicts}>{(path) => <FileRow mark="C" path={path} />}</For>
            <Show when={preview.conflicts.length > 0}>
              <InspectorNote>{t("mergePreview.resolve").replace("{base}", preview.baseBranch)}</InspectorNote>
            </Show>
            <Show when={!preview.remoteTracking}>
              <InspectorNote tone="warn">{t("mergePreview.localOnly")}</InspectorNote>
            </Show>
          </>
        )}
      </Show>
      <Show when={props.error}>{(error) => <InspectorNote tone="error">{error()}</InspectorNote>}</Show>
      <Button size="sm" icon={props.result ? "refresh" : undefined} loading={props.busy}
        loadingLabel={t("mergePreview.checking")} onClick={props.onCheck}>
        {t(props.result || props.error ? "mergePreview.again" : "mergePreview.check")}
      </Button>
    </InspectorSection>
  );
}
