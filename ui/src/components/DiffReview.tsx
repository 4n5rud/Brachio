import { SegmentedControl } from "./ui/SegmentedControl";
import { Button } from "./ui/Button";
import { For, Show, createMemo, createSignal } from "solid-js";
import { t } from "../i18n";
import { diffRows, splitDiffRows, type DiffLocation, type DiffRow } from "../lib/diff";

export function DiffReview(props: {
  patch: string;
  onDraft: (text: string) => Promise<boolean>;
}) {
  const rows = createMemo(() => diffRows(props.patch));
  const [split, setSplit] = createSignal(false);
  const paired = createMemo(() => split() ? splitDiffRows(rows()) : []);
  const [location, setLocation] = createSignal<DiffLocation>();
  const [comment, setComment] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [error, setError] = createSignal("");

  const draft = async (target: DiffLocation) => {
    const text = comment().trim();
    if (!text || sending()) return;
    setSending(true);
    setError("");
    try {
      const side = t(target.side === "old" ? "diff.oldLine" : "diff.newLine");
      if (await props.onDraft(`[${target.path}:${target.line} · ${side}] ${text}`)) {
        setComment("");
        setLocation(undefined);
      } else {
        setError(t("diff.notReady"));
      }
    } catch (failure) {
      setError(String(failure));
    } finally {
      setSending(false);
    }
  };

  const line = (row: DiffRow) => (
    <div class="diff-row">
      <Show when={row.location} fallback={<span aria-hidden="true" />}>
        {(target) => (
          <button type="button" disabled={sending()}
            aria-label={`${t("diff.comment")} ${target().path}:${target().line} ${t(target().side === "old" ? "diff.oldLine" : "diff.newLine")}`}
            aria-pressed={location() === target()}
            onClick={() => { setLocation(target()); setError(""); }}>
            {target().line}
          </button>
        )}
      </Show>
      <code>{row.text}</code>
    </div>
  );

  return (
    <>
      <SegmentedControl class="diff-layout" label={t("diff.layout")} value={split() ? "split" : "unified"}
        onChange={(value) => setSplit(value === "split")}
        options={[
          { value: "unified", label: t("diff.unified") },
          { value: "split", label: t("diff.split") },
        ]} />
      <div class="inspector-patch" classList={{ "inspector-patch-split": split() }}
        tabIndex={0} aria-label={t("inspector.changes")}>
        <Show when={split()} fallback={<For each={rows()}>{line}</For>}>
          <div class="diff-pair diff-column-head">
            <span>{t("diff.oldLine")}</span><span>{t("diff.newLine")}</span>
          </div>
          <For each={paired()}>{(row) => "shared" in row ? line(row.shared) : (
            <div class="diff-pair">
              <div class="diff-removed">{row.old && line(row.old)}</div>
              <div class="diff-added">{row.next && line(row.next)}</div>
            </div>
          )}</For>
        </Show>
      </div>
      <Show when={location()}>
        {(target) => (
          <form class="diff-comment" onSubmit={(event) => {
            event.preventDefault();
            void draft(target());
          }}>
            <label>
              {t("diff.comment")} · {target().path}:{target().line}
              <textarea rows={3} maxLength={2000} disabled={sending()} value={comment()}
                onInput={(event) => setComment(event.currentTarget.value)} />
            </label>
            <Button size="sm" type="submit" disabled={sending() || !comment().trim()}>
              {t("diff.insert")}
            </Button>
            <Show when={error()}><p role="alert">{error()}</p></Show>
          </form>
        )}
      </Show>
    </>
  );
}
