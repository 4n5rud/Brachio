import { Input } from "./ui/Input";
import { NameDialog } from "./NameDialog";
import { copyPath } from "./ui/PathLink";
import { openInEditor } from "../lib/editor";
import { noteRecentFile } from "../lib/recent-files";
import { revealPath } from "../lib/workspace";
import type { FileAction } from "./FileTree";
import { IconButton } from "./ui/IconButton";
import { FileTree } from "./FileTree";
import { InspectorNote, OutputLog } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import { Show, createEffect, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import { confirmDialog } from "./ui/ConfirmDialog";
import { pushToast } from "../lib/toasts";
import { registerTreeDrop } from "../lib/tree-drop";
import type { TerminalDrop } from "../lib/terminal-drop";
import { quoteShellPaths } from "../lib/shell-quote";
import { shellKind } from "../lib/terminal";
import { t } from "../i18n";
import { useAsyncAction } from "../lib/async-action";
import { copyIntoTask, createTaskEntry, listTaskFiles, moveTaskPath, previewTaskFile, previewTaskMedia, renameTaskPath, taskCopyConflicts, trashTaskPath } from "../lib/workspace";

const RESULT_LIMIT = 100;
const mediaTypes = new Map([
  ["png", "image/png"], ["jpg", "image/jpeg"], ["jpeg", "image/jpeg"],
  ["gif", "image/gif"], ["webp", "image/webp"], ["pdf", "application/pdf"],
]);

export interface FilePreview {
  loading?: boolean;
  error?: string;
  /** Null for a file that is not UTF-8 text. */
  text?: string | null;
  truncated?: boolean;
  media?: { url: string; type: string };
}

/** Search box and matching files, then the chosen file under them. */
export function FilesPanel(props: {
  filter: string;
  onFilter: (value: string) => void;
  loading: boolean;
  error?: string;
  matches: readonly string[];
  /** Keeps which folders are open for this task. */
  treeKey?: string;
  selected: string;
  onSelect: (path: string) => void;
  onRefresh: () => void;
  preview?: FilePreview;
  onMove?: (from: string, toDir: string) => void;
  onTerminalDrop?: (path: string, terminal: TerminalDrop) => void;
  onAction?: (action: FileAction, path: string, kind: "file" | "dir") => void;
}) {
  return (
    <>
      <InspectorSection title={t("files.title")}
        end={<IconButton size="sm" icon="refresh" label={t("inspector.refresh")} busy={props.loading} onClick={props.onRefresh} />}>
        <Input aria-label={t("files.filter")} placeholder={t("files.filter")} mono value={props.filter}
          onInput={(event) => props.onFilter(event.currentTarget.value)} />
        <Show when={props.error}>{(error) => <InspectorNote tone="error">{error()}</InspectorNote>}</Show>
        <Show when={!props.loading && !props.error && props.matches.length === 0}>
          <InspectorNote>{t("files.empty")}</InspectorNote>
        </Show>
        <FileTree paths={props.matches} query={props.filter.trim()} treeKey={props.treeKey ?? ""}
          selected={props.selected} onSelect={props.onSelect} limit={RESULT_LIMIT}
          onMove={props.onMove} onTerminalDrop={props.onTerminalDrop} onAction={props.onAction} />
        <Show when={props.filter.trim() && props.matches.length > RESULT_LIMIT}><InspectorNote>{t("files.more")}</InspectorNote></Show>
      </InspectorSection>
      <Show when={props.preview}>
        {(preview) => (
          <InspectorSection title={props.selected}>
            <Show when={preview().loading}><InspectorNote tone="progress">{t("files.loading")}</InspectorNote></Show>
            <Show when={preview().media} keyed>
              {(item) => item.type === "application/pdf"
                ? <iframe class="file-preview-pdf" src={item.url} title={props.selected} />
                : <img class="file-preview-image" src={item.url} alt={props.selected} />}
            </Show>
            <Show when={preview().error}>{(error) => <InspectorNote tone="error">{error()}</InspectorNote>}</Show>
            <Show when={preview().text !== undefined && !preview().loading}>
              <Show when={preview().text !== null} fallback={<InspectorNote>{t("files.binary")}</InspectorNote>}>
                <OutputLog text={preview().text ?? ""} />
              </Show>
              <Show when={preview().truncated}><InspectorNote>{t("files.truncated")}</InspectorNote></Show>
            </Show>
          </InspectorSection>
        )}
      </Show>
    </>
  );
}

/** Lists the task's files; choosing one previews it and adds @path to the agent prompt. */
export function TaskFiles(props: { taskId: string; /** The task folder on disk, for copy and show-in-folder. */ root?: string; onInsert: (path: string) => void }) {
  const [naming, setNaming] = createSignal<{ kind: "rename" | "newFile" | "newFolder"; path: string }>();
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

  const previewState = (): FilePreview | undefined => {
    if (!path()) return undefined;
    if (preview.loading) return { loading: true };
    const result = preview();
    if (result?.path !== path()) return undefined;
    return { error: result.error, text: result.file?.text, truncated: result.file?.truncated, media: media() };
  };
  const choose = (file: string) => {
    setPath(file);
    props.onInsert(file);
  };

  const failed = (title: string, failure: unknown) =>
    pushToast({ tone: "error", title, message: String(failure) });
  const move = async (from: string, toDir: string) => {
    try {
      const next = await moveTaskPath(props.taskId, from, toDir);
      if (path() === from || path().startsWith(`${from}/`)) setPath(next + path().slice(from.length));
      await refetch();
      pushToast({ tone: "success", message: t("files.moved").replace("{path}", () => next) });
    } catch (failure) {
      failed(t("files.moveFailed"), failure);
    }
  };
  // Agents take an @path reference in their own input; a shell gets the path quoted for it.
  const dropOnTerminal = async (file: string, terminal: TerminalDrop) => {
    try {
      if (terminal.kind === "agent") {
        props.onInsert(/[\s"]/u.test(file) ? `"${file.replaceAll('"', '\\"')}"` : file);
      } else {
        await terminal.target.insert(quoteShellPaths(await shellKind(), [file]));
      }
    } catch (failure) {
      failed(t("terminal.inputFailed"), failure);
    }
  };
  const copyInto = async (dir: string, sources: string[]) => {
    const folder = dir || t("files.dropTargetRoot");
    try {
      const clashes = await taskCopyConflicts(props.taskId, sources, dir);
      if (clashes.length > 0) {
        const replace = await confirmDialog({
          title: t("files.overwriteTitle"),
          body: t("files.overwriteBody").replace("{folder}", () => folder).replace("{names}", () => clashes.join("\n")),
          confirmLabel: t("files.overwriteConfirm"),
        });
        if (!replace) return;
      }
      const made = await copyIntoTask(props.taskId, sources, dir, clashes.length > 0);
      await refetch();
      pushToast({ tone: "success", message: t("files.copied").replace("{count}", String(made.length)).replace("{folder}", () => folder) });
    } catch (failure) {
      failed(t("files.copyFailed"), failure);
    }
  };
  const reload = useAsyncAction(async () => { await refetch(); }, { failureToast: true });
  onCleanup(registerTreeDrop({ copyInto }));

  const absolute = (file: string) => {
    const root = props.root ?? "";
    const joined = `${root.replace(/[\\/]+$/u, "")}/${file}`;
    return root.includes("\\") ? joined.replaceAll("/", "\\") : joined;
  };
  const rename = async (from: string, name: string) => {
    try {
      const next = await renameTaskPath(props.taskId, from, name);
      if (path() === from || path().startsWith(`${from}/`)) setPath(next + path().slice(from.length));
      await refetch();
    } catch (failure) {
      failed(t("files.renameFailed"), failure);
    }
  };
  const create = async (dir: string, name: string, folder: boolean) => {
    try {
      const made = await createTaskEntry(props.taskId, dir, name, folder);
      await refetch();
      pushToast({ tone: "success", message: t(folder ? "files.folderCreated" : "files.created").replace("{path}", () => made) });
      if (!folder) choose(made);
    } catch (failure) {
      failed(t("files.createFailed"), failure);
    }
  };
  const onAction = async (action: FileAction, target: string, kind: "file" | "dir") => {
    if (action === "open") {
      noteRecentFile(props.taskId, target);
      await openInEditor(props.taskId, target).catch((failure) => failed(t("files.openFailed"), failure));
    } else if (action === "reveal") {
      await revealPath(absolute(target)).catch((failure) => failed(t("files.revealFailed"), failure));
    } else if (action === "copyPath") {
      await copyPath(absolute(target));
    } else if (action === "copyRelative") {
      await copyPath(target);
    } else if (action === "trash") {
      const ok = await confirmDialog({
        title: t("files.trashTitle").replace("{name}", () => target.slice(target.lastIndexOf("/") + 1)),
        body: t(kind === "dir" ? "files.trashFolderBody" : "files.trashBody"),
        confirmLabel: t("files.trash"),
      });
      if (!ok) return;
      try {
        await trashTaskPath(props.taskId, target);
        if (path() === target || path().startsWith(`${target}/`)) setPath("");
        await refetch();
        pushToast({ tone: "success", message: t("files.trashed") });
      } catch (failure) {
        failed(t("files.trashFailed"), failure);
      }
    } else {
      setNaming({ kind: action, path: target });
    }
  };

  return (
    <>
    <FilesPanel filter={filter()} onFilter={setFilter} loading={files.loading || reload.pending()} error={files()?.error}
      treeKey={props.taskId} matches={matches()} selected={path()} onSelect={choose} onRefresh={() => void reload.run()} preview={previewState()}
      onMove={(from, toDir) => void move(from, toDir)} onTerminalDrop={(file, terminal) => void dropOnTerminal(file, terminal)}
      onAction={(action, target, kind) => void onAction(action, target, kind)} />
    <Show when={naming()}>
      {(request) => (
        <NameDialog
          title={t(request().kind === "rename" ? "files.renameTitle" : request().kind === "newFile" ? "files.newFile" : "files.newFolder")}
          label={t("files.nameLabel")}
          initial={request().kind === "rename" ? request().path.slice(request().path.lastIndexOf("/") + 1) : ""}
          confirmLabel={t(request().kind === "rename" ? "files.rename" : "files.create")}
          onClose={() => setNaming(undefined)}
          onSubmit={(name) => {
            const current = request();
            setNaming(undefined);
            if (current.kind === "rename") void rename(current.path, name);
            else void create(current.path, name, current.kind === "newFolder");
          }} />
      )}
    </Show>
    </>
  );
}
