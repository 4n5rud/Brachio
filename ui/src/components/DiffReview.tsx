import { SegmentedControl } from "./ui/SegmentedControl";
import { Button } from "./ui/Button";
import { TextArea } from "./ui/Input";
import { DiffStat } from "./ui/DiffStat";
import { For, Show, createMemo, createSignal, type JSX } from "solid-js";
import { t } from "../i18n";
import { takeDraftProblem } from "../lib/draft-problem";
import { hunkPatch } from "../lib/diff-hunks";
import { IconButton } from "./ui/IconButton";
import { diffFiles, diffRows, splitDiffRows, type DiffFile, type DiffLocation, type DiffRow } from "../lib/diff";
import { InspectorNote } from "./InspectorNote";
import "./DiffReview.css";

type Line =
  | { kind: "file"; file: DiffFile }
  | { kind: "code" | "note"; row: DiffRow }
  | { kind: "hunk"; row: DiffRow; file: number; hunk: number };

type Item = Line | { kind: "pair"; old?: DiffRow; next?: DiffRow };

const HEADER =
  /^(index |--- |\+\+\+ |(old|new|deleted file|new file) mode|similarity index|dissimilarity index|rename (from|to) |copy (from|to) )/;

export function DiffReview(props: {
  patch: string;
  split: boolean;
  onSplit: (split: boolean) => void;
  onDraft: (text: string) => Promise<boolean>;
  /** Throws away one hunk; given its patch with the file header. */
  onDiscardHunk?: (patch: string) => void;
  /** A line already chosen, for the component catalog. */
  initial?: { line?: number; side?: "old" | "new"; comment?: string; error?: string };
}) {
  const rows = createMemo(() => diffRows(props.patch));
  const lines = createMemo<Line[]>(() => {
    const files = diffFiles(props.patch);
    let next = 0;
    let hunks = 0;
    const result: Line[] = [];
    for (const row of rows()) {
      if (row.location) result.push({ kind: "code", row });
      else if (row.text.startsWith("diff --git ")) {
        const file = files[next++];
        hunks = 0;
        if (file) result.push({ kind: "file", file });
      } else if (row.text.startsWith("@@")) result.push({ kind: "hunk", row, file: next - 1, hunk: hunks++ });
      else if (row.text && !HEADER.test(row.text)) result.push({ kind: "note", row });
    }
    return result;
  });
  const items = createMemo<Item[]>(() => {
    if (!props.split) return [];
    const grouped: Item[] = [];
    let pending: DiffRow[] = [];
    const flush = () => {
      for (const item of splitDiffRows(pending)) {
        if ("shared" in item) grouped.push({ kind: "code", row: item.shared });
        else grouped.push({ kind: "pair", old: item.old, next: item.next });
      }
      pending = [];
    };
    for (const line of lines()) {
      if (line.kind === "code") pending.push(line.row);
      else {
        flush();
        grouped.push(line);
      }
    }
    flush();
    return grouped;
  });
  const [location, setLocation] = createSignal<DiffLocation | undefined>(
    props.initial?.line === undefined ? undefined
      : rows().find((row) => row.location?.line === props.initial?.line && row.location?.side === (props.initial?.side ?? "new"))?.location,
  );
  const [comment, setComment] = createSignal(props.initial?.comment ?? "");
  const [sending, setSending] = createSignal(false);
  const [error, setError] = createSignal(props.initial?.error ?? "");

  const close = () => {
    setLocation(undefined);
    setError("");
  };

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
        setError(takeDraftProblem() ?? "");
      }
    } catch (failure) {
      setError(String(failure));
    } finally {
      setSending(false);
    }
  };

  const form = (target: DiffLocation) => (
    <form class="comment-box" onSubmit={(event) => {
      event.preventDefault();
      void draft(target);
    }} onKeyDown={(event) => { if (event.key === "Escape") close(); }}>
      <span class="where">
        {target.path}:{target.line} {t(target.side === "old" ? "diff.oldLine" : "diff.newLine")}
      </span>
      <TextArea aria-label={t("diff.comment")} rows={3} maxLength={2000} disabled={sending()} value={comment()}
        ref={(field) => queueMicrotask(() => field.focus())}
        onInput={(event) => setComment(event.currentTarget.value)} />
      <Show when={error()}><InspectorNote tone="error">{error()}</InspectorNote></Show>
      <div class="row-gap">
        <Button variant="primary" size="sm" type="submit" loading={sending()} loadingLabel={t("diff.inserting")}
          disabled={!comment().trim()}>
          {t("diff.insert")}
        </Button>
        <Button variant="ghost" size="sm" disabled={sending()} onClick={close}>{t("diff.cancel")}</Button>
      </div>
    </form>
  );

  const code = (row: DiffRow): JSX.Element => {
    const target = row.location;
    return (
      <>
        <div class="drow" classList={{
          add: row.location?.side === "new" && row.text.startsWith("+"),
          del: row.location?.side === "old",
          sel: !!target && location() === target,
        }}>
          <Show when={target} fallback={<span aria-hidden="true" />}>
            {(point) => (
              <button type="button" class="ln" disabled={sending()}
                aria-label={`${t("diff.comment")} ${point().path}:${point().line} ${t(point().side === "old" ? "diff.oldLine" : "diff.newLine")}`}
                aria-pressed={location() === point()}
                onClick={() => { setLocation(point()); setError(""); }}>
                {point().line}
              </button>
            )}
          </Show>
          <span class="sg">{row.text.slice(0, 1)}</span>
          <span>{row.text.slice(1)}</span>
        </div>
        <Show when={!props.split && target && location() === target}>{form(target!)}</Show>
      </>
    );
  };

  const other = (line: Line): JSX.Element => {
    if (line.kind === "file") {
      return (
        <div class="diff-file">
          <span class={`fstat fstat-${line.file.status.toLowerCase()}`}>{line.file.status}</span>
          <span class="diff-file-path" title={line.file.path}>{line.file.path}</span>
          <DiffStat additions={line.file.additions} deletions={line.file.deletions} />
        </div>
      );
    }
    if (line.kind === "code") return code(line.row);
    return (
      <div class={line.kind === "hunk" ? "drow hunk" : "drow note"}>
        <span /><span />
        <span>
          {line.row.text}
          <Show when={line.kind === "hunk" && props.onDiscardHunk}>
            <IconButton size="sm" icon="x" label={t("changes.discardHunk")}
              onClick={() => {
                const patch = line.kind === "hunk" ? hunkPatch(props.patch, line.file, line.hunk) : undefined;
                if (patch) props.onDiscardHunk?.(patch);
              }} />
          </Show>
        </span>
      </div>
    );
  };

  const selectedIn = (...rows: (DiffRow | undefined)[]) =>
    rows.find((row) => row?.location && row.location === location())?.location;

  return (
    <>
      <SegmentedControl class="diff-layout" label={t("diff.layout")} value={props.split ? "split" : "unified"}
        onChange={(value) => props.onSplit(value === "split")}
        options={[
          { value: "unified", label: t("diff.unified") },
          { value: "split", label: t("diff.split") },
        ]} />
      <div class="diff" classList={{ "diff-split": props.split }} tabIndex={0} aria-label={t("inspector.changes")}>
        <Show when={props.split} fallback={<For each={lines()}>{other}</For>}>
          <For each={items()}>{(item) => item.kind === "pair" ? (
            <>
              <div class="diff-pair">
                <div class="diff-side">{item.old && code(item.old)}</div>
                <div class="diff-side">{item.next && code(item.next)}</div>
              </div>
              <Show when={selectedIn(item.old, item.next)}>{(target) => form(target())}</Show>
            </>
          ) : other(item)}</For>
        </Show>
      </div>
    </>
  );
}
