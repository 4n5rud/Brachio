import { For, Show, createMemo, createSignal } from "solid-js";
import { t } from "../i18n";
import { diffRows, type DiffLocation } from "../lib/diff";

export function DiffReview(props: {
  patch: string;
  onDraft: (text: string) => Promise<boolean>;
}) {
  const rows = createMemo(() => diffRows(props.patch));
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

  return (
    <>
      <div class="inspector-patch" tabIndex={0} aria-label={t("inspector.changes")}>
        <For each={rows()}>
          {(row) => (
            <div class="diff-row">
              <Show when={row.location} fallback={<span aria-hidden="true" />}>
                {(target) => (
                  <button type="button" disabled={sending()}
                    aria-label={`${t("diff.comment")} ${target().path}:${target().line}`}
                    aria-pressed={location() === target()}
                    onClick={() => { setLocation(target()); setError(""); }}>
                    {target().line}
                  </button>
                )}
              </Show>
              <code>{row.text}</code>
            </div>
          )}
        </For>
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
            <button type="submit" class="button button-sm" disabled={sending() || !comment().trim()}>
              {t("diff.insert")}
            </button>
            <Show when={error()}><p role="alert">{error()}</p></Show>
          </form>
        )}
      </Show>
    </>
  );
}
