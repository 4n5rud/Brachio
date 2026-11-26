import { Transaction, type EditorState, type StateEffect, type Text, type TransactionSpec } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { Match, Show, Switch, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { createEditorState, languageSlot, loadLanguage } from "../lib/code-editor";
import { openInEditor } from "../lib/editor";
import { AUTOSAVE_DELAY_MS, editorAutosaveEnabled } from "../lib/editor-settings";
import { editorLanguageOf, middleChange } from "../lib/editor-text";
import {
  checkEditableFile,
  fileEditProblem,
  readEditableFile,
  readFileHead,
  writeEditableFile,
  type FileCheck,
  type FileVersion,
  type LineEnding,
} from "../lib/file-edit";
import { openFileWatch } from "../lib/open-file-watch";
import { isMac } from "../lib/shortcut";
import type { TabSurfaceProps } from "./TabSurface";
import { Button } from "./ui/Button";
import { confirmDialog } from "./ui/ConfirmDialog";
import { Notice } from "./ui/Notice";
import "./FileTab.css";

/** How the file was opened: editable and saved back the way it was read, or shown read-only. */
type Opened =
  | { kind: "edit"; version: FileVersion; lineEnding: LineEnding; bom: boolean; /** The text on disk. */ baseline: Text }
  | { kind: "readOnly"; reason: "too_large" | "mixed_endings"; truncated: boolean; size: number };

type DiskNotice =
  | { kind: "changed" }
  | { kind: "missing" }
  | { kind: "conflict" }
  | { kind: "saveFailed"; message: string }
  | { kind: "reloadFailed"; message: string };

/** What a file tab keeps while its view is released: the editor state holds text, cursor, folds and undo. */
interface Session {
  marker: typeof SESSION;
  state: EditorState;
  opened: Opened;
  notice?: DiskNotice;
  scroll?: StateEffect<unknown>;
  languageLoaded: boolean;
}

const SESSION = Symbol("file-tab-session");
const isSession = (value: unknown): value is Session =>
  typeof value === "object" && value !== null && (value as { marker?: unknown }).marker === SESSION;

type Phase =
  | { kind: "loading" }
  | { kind: "ready"; session: Session }
  | { kind: "notText" }
  | { kind: "failed"; message: string };

const MIB = 1024 * 1024;
/** How long "Saved" stays in the tab's status line. */
const SAVED_SHOWN_MS = 3_000;

/**
 * A task's file in a code editor. Text is edited as read and written back with the line ending and
 * byte order mark it had; saving is Ctrl+S (Cmd+S on macOS), or a pause in typing with autosave on.
 */
export function FileTab(props: TabSurfaceProps) {
  const path = () => props.tab.target ?? "";
  const restored = isSession(props.state) ? props.state : undefined;
  let session = restored;
  let view: EditorView | undefined;
  let disposed = false;
  let stopWatch: (() => void) | undefined;
  let autosaveTimer: ReturnType<typeof setTimeout> | undefined;
  const [phase, setPhase] = createSignal<Phase>(restored ? { kind: "ready", session: restored } : { kind: "loading" });
  const [notice, setNotice] = createSignal<DiskNotice | undefined>(restored?.notice);
  const [saving, setSaving] = createSignal(false);
  /** How the shown file was opened, for the notices and the status line. */
  const [info, setInfo] = createSignal<Opened | undefined>(restored?.opened);
  const [justSaved, setJustSaved] = createSignal(false);
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  const showNotice = (next: DiskNotice | undefined) => {
    setNotice(next);
    if (session) session.notice = next;
  };
  const current = (): EditorState | undefined => view?.state ?? session?.state;
  const isDirty = (): boolean => {
    const state = current();
    return session?.opened.kind === "edit" && state !== undefined && !state.doc.eq(session.opened.baseline);
  };
  /** Applies a change whether or not the view is mounted. */
  const apply = (spec: TransactionSpec) => {
    if (view) view.dispatch(spec);
    else if (session) session.state = session.state.update(spec).state;
  };

  const sessionFor = (text: string, opened: Omit<Extract<Opened, { kind: "edit" }>, "baseline"> | Extract<Opened, { kind: "readOnly" }>): Session => {
    const state = createEditorState(text, { readOnly: opened.kind === "readOnly" });
    return {
      marker: SESSION,
      state,
      opened: opened.kind === "edit" ? { ...opened, baseline: state.doc } : opened,
      languageLoaded: false,
    };
  };

  const open = async (): Promise<Phase> => {
    try {
      const file = await readEditableFile(props.taskId, path());
      return {
        kind: "ready",
        session: sessionFor(file.text, { kind: "edit", version: file.version, lineEnding: file.lineEnding, bom: file.bom }),
      };
    } catch (failure) {
      const { problem, message } = fileEditProblem(failure);
      if (problem === "not_text") return { kind: "notText" };
      if (problem !== "too_large" && problem !== "mixed_endings") return { kind: "failed", message };
      try {
        const head = await readFileHead(props.taskId, path());
        return {
          kind: "ready",
          session: sessionFor(head.text, { kind: "readOnly", reason: problem, truncated: head.truncated, size: head.size }),
        };
      } catch (headFailure) {
        const second = fileEditProblem(headFailure);
        return second.problem === "not_text" ? { kind: "notText" } : { kind: "failed", message: second.message };
      }
    }
  };
  /** Drops the editor view and the disk watch, keeping nothing: the file is read again. */
  const release = () => {
    stopWatch?.();
    stopWatch = undefined;
    view?.destroy();
    view = undefined;
  };
  const load = async () => {
    release();
    session = undefined;
    setPhase({ kind: "loading" });
    const next = await open();
    if (disposed) {
      // The view was released while reading; keep what was read for the next time it is shown.
      if (next.kind === "ready") props.saveState(next.session);
      return;
    }
    if (next.kind === "ready") session = next.session;
    setPhase(next);
  };
  if (!restored) void load();

  const reportDirty = () => {
    const dirty = isDirty();
    props.setDirty(dirty);
    clearTimeout(autosaveTimer);
    if (dirty && editorAutosaveEnabled()) autosaveTimer = setTimeout(() => void save(), AUTOSAVE_DELAY_MS);
  };

  const save = async () => {
    const owned = session;
    const state = current();
    if (!owned || !state || owned.opened.kind !== "edit" || saving()) return;
    const opened = owned.opened;
    if (state.doc.eq(opened.baseline)) return;
    setSaving(true);
    try {
      const version = await writeEditableFile(props.taskId, path(), state.doc.toString(), opened.version, opened.lineEnding, opened.bom);
      if (owned.opened === opened) owned.opened = { ...opened, version, baseline: state.doc };
      if (session === owned) setInfo(owned.opened);
      openFileWatch.accept(props.taskId, path(), version);
      if (session === owned) {
        showNotice(undefined);
        setJustSaved(true);
        clearTimeout(savedTimer);
        savedTimer = setTimeout(() => setJustSaved(false), SAVED_SHOWN_MS);
      } else {
        owned.notice = undefined;
      }
      reportDirty();
    } catch (failure) {
      const { problem, message } = fileEditProblem(failure);
      const next: DiskNotice = problem === "conflict" ? { kind: "conflict" } : { kind: "saveFailed", message };
      if (session === owned) showNotice(next); else owned.notice = next;
    } finally {
      setSaving(false);
    }
  };

  /** Runs `step` now, or once a composition (Korean, Japanese, Chinese input) in progress ends. */
  const afterComposition = (step: () => void) => {
    const editor = view;
    if (!editor?.composing) {
      step();
      return;
    }
    editor.contentDOM.addEventListener("compositionend", () => setTimeout(() => afterComposition(step), 0), { once: true });
  };

  /** Reads the file again and applies only the part that differs. Without `discard` it never drops unsaved edits. */
  const reload = async (discard = false) => {
    const owned = session;
    if (!owned) return;
    if (owned.opened.kind === "readOnly") {
      if (!disposed) await load();
      return;
    }
    try {
      const file = await readEditableFile(props.taskId, path());
      if (session !== owned) return;
      afterComposition(() => {
        if (session !== owned) return;
        if (!discard && isDirty()) {
          showNotice({ kind: "changed" });
          return;
        }
        const state = current();
        if (!state) return;
        const change = middleChange(state.doc.toString(), file.text);
        if (change) apply({ changes: change, annotations: Transaction.addToHistory.of(false) });
        owned.opened = { kind: "edit", version: file.version, lineEnding: file.lineEnding, bom: file.bom, baseline: current()?.doc ?? state.doc };
        setInfo(owned.opened);
        openFileWatch.accept(props.taskId, path(), file.version);
        showNotice(undefined);
        reportDirty();
      });
    } catch (failure) {
      if (session === owned) showNotice({ kind: "reloadFailed", message: fileEditProblem(failure).message });
    }
  };
  const discardAndReload = async () => {
    const ok = await confirmDialog({
      title: t("editor.reloadTitle").replace("{name}", () => fileName(path())),
      body: t("editor.reloadBody"),
      confirmLabel: t("editor.reloadConfirm"),
    });
    if (ok) await reload(true);
  };

  const onDisk = (check: FileCheck) => {
    if (check.status === "missing") showNotice({ kind: "missing" });
    else if (check.status === "changed") {
      if (isDirty()) showNotice({ kind: "changed" });
      else void reload();
    }
  };

  const mount = (parent: HTMLElement, owned: Session) => {
    release();
    setInfo(owned.opened);
    view = new EditorView({
      state: owned.state,
      parent,
      dispatchTransactions: (transactions, editor) => {
        editor.update(transactions);
        if (transactions.some((transaction) => transaction.docChanged)) reportDirty();
      },
    });
    if (owned.scroll) view.dispatch({ effects: owned.scroll });
    const language = editorLanguageOf(path());
    if (language && !owned.languageLoaded) {
      owned.languageLoaded = true;
      loadLanguage(language).then(
        (extension) => apply({ effects: languageSlot.reconfigure(extension) }),
        () => { owned.languageLoaded = false; },
      );
    }
    if (owned.opened.kind === "edit") {
      stopWatch = openFileWatch.watch(props.taskId, path(), owned.opened.version, onDisk);
      // A tab shown again checks at once: the file may have changed while it was hidden.
      if (owned === restored) {
        void checkEditableFile(props.taskId, path(), owned.opened.version, true)
          .then((check) => { if (!disposed && session === owned) onDisk(check); }, () => {});
        reportDirty();
      }
    }
    if (owned !== restored) queueMicrotask(() => view?.focus());
  };

  onCleanup(() => {
    disposed = true;
    stopWatch?.();
    clearTimeout(autosaveTimer);
    clearTimeout(savedTimer);
    if (!view || !session) return;
    session.state = view.state;
    session.scroll = view.scrollSnapshot();
    view.destroy();
    view = undefined;
    props.saveState(session);
  });

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.isComposing || view?.composing) return;
    const primary = isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
    if (primary && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void save();
    }
  };

  const readOnly = () => {
    const opened = info();
    return opened?.kind === "readOnly" ? opened : undefined;
  };
  const editable = () => {
    const opened = info();
    return opened?.kind === "edit" ? opened : undefined;
  };
  const readySession = () => {
    const current = phase();
    return current.kind === "ready" ? current.session : undefined;
  };
  const failure = () => {
    const current = phase();
    return current.kind === "failed" ? current.message : undefined;
  };

  return (
    <div class="file-tab" onKeyDown={onKeyDown}>
      <Switch>
        <Match when={phase().kind === "loading"}>
          <div class="tab-surface-placeholder" role="status" aria-busy="true" />
        </Match>
        <Match when={phase().kind === "notText"}>
          <div class="tab-surface-placeholder" role="status">
            <strong>{fileName(path())}</strong>
            <span>{t("editor.notText")}</span>
            <Button size="sm" onClick={() => void openInEditor(props.taskId, path())}>{t("files.openEditor")}</Button>
          </div>
        </Match>
        <Match when={failure()}>
          {(message) => (
            <div class="tab-surface-placeholder" role="alert">
              <strong>{fileName(path())}</strong>
              <span>{t("editor.openFailed")}</span>
              <code>{message()}</code>
              <Button size="sm" onClick={() => void load()}>{t("editor.retry")}</Button>
            </div>
          )}
        </Match>
        <Match when={readySession()} keyed>
          {(owned) => (
            <>
              <Show when={readOnly()}>
                {(info) => (
                  <Notice tone="info" lead={t("editor.readOnly")}>
                    {info().reason === "mixed_endings" ? t("editor.mixedEndings")
                      : info().truncated ? t("editor.tooLargeHead").replace("{size}", formatMib(info().size)).replace("{limit}", "2")
                      : t("editor.tooLarge")}
                  </Notice>
                )}
              </Show>
              <Show when={notice()}>
                {(current) => (
                  <Notice tone={current().kind === "changed" || current().kind === "missing" ? "blocked" : "error"}
                    lead={noticeLead(current())}
                    actions={
                      <Show when={current().kind !== "missing"}>
                        <Button size="sm" onClick={() => void (isDirty() ? discardAndReload() : reload())}>{t("editor.reload")}</Button>
                      </Show>
                    }>
                    {noticeText(current())}
                  </Notice>
                )}
              </Show>
              <div class="file-editor" ref={(element) => queueMicrotask(() => {
                // Mounted once the element is in the page, so the editor measures real sizes.
                if (!disposed && session === owned) mount(element, owned);
              })} />
              <div class="file-tab-bar">
                <span class="file-tab-path" title={path()}>{path()}</span>
                <span class="file-tab-status" role="status">
                  {saving() ? t("editor.saving") : justSaved() ? t("editor.saved") : ""}
                </span>
                <Show when={editable()}>
                  {(info) => (
                    <>
                      <span>{info().lineEnding === "crlf" ? "CRLF" : "LF"}</span>
                      <span>{info().bom ? t("editor.utf8Bom") : "UTF-8"}</span>
                    </>
                  )}
                </Show>
              </div>
            </>
          )}
        </Match>
      </Switch>
    </div>
  );
}

const fileName = (path: string) => path.replaceAll("\\", "/").split("/").pop() || path;

const formatMib = (bytes: number) => (bytes / MIB).toFixed(1);

function noticeLead(notice: DiskNotice): string {
  switch (notice.kind) {
    case "changed": return t("editor.changedLead");
    case "missing": return t("editor.missingLead");
    case "conflict": return t("editor.conflictLead");
    case "saveFailed": return t("editor.saveFailed");
    case "reloadFailed": return t("editor.reloadFailed");
  }
}

function noticeText(notice: DiskNotice): string {
  switch (notice.kind) {
    case "changed": return t("editor.changedBody");
    case "missing": return t("editor.missingBody");
    case "conflict": return t("editor.conflictBody");
    case "saveFailed": return notice.message;
    case "reloadFailed": return notice.message;
  }
}
