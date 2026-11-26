import { Input } from "./ui/Input";
import { NameDialog } from "./NameDialog";
import { copyPath } from "./ui/PathLink";
import { openInEditor } from "../lib/editor";
import { noteRecentFile } from "../lib/recent-files";
import { requestContentTab } from "../lib/tab-request";
import { revealPath } from "../lib/workspace";
import type { FileAction } from "./FileTree";
import { IconButton } from "./ui/IconButton";
import { FileTree } from "./FileTree";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import { Show, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import { confirmDialog } from "./ui/ConfirmDialog";
import { pushToast } from "../lib/toasts";
import { pushUndoToast } from "../lib/undo";
import { registerTreeDrop } from "../lib/tree-drop";
import type { TerminalDrop } from "../lib/terminal-drop";
import { quoteShellPaths } from "../lib/shell-quote";
import { shellKind } from "../lib/terminal";
import { t } from "../i18n";
import { useAsyncAction } from "../lib/async-action";
import { copyIntoTask, createTaskEntry, listTaskFiles, moveTaskPath, renameTaskPath, restoreTaskPath, taskCopyConflicts, trashTaskPath } from "../lib/workspace";

const RESULT_LIMIT = 100;
/** Search box and the matching files as a folder tree. */
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
  onOpen?: (path: string) => void;
  onRefresh: () => void;
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
          selected={props.selected} onSelect={props.onSelect} onOpen={props.onOpen} limit={RESULT_LIMIT}
          onMove={props.onMove} onTerminalDrop={props.onTerminalDrop} onAction={props.onAction} />
        <Show when={props.filter.trim() && props.matches.length > RESULT_LIMIT}><InspectorNote>{t("files.more")}</InspectorNote></Show>
      </InspectorSection>
    </>
  );
}

/**
 * Lists the task's files. A click shows a file in a temporary tab, a double click keeps the tab,
 * and the menu adds @path to the agent prompt.
 */
export function TaskFiles(props: { taskId: string; /** The task folder on disk, for copy and show-in-folder. */ root?: string; onInsert: (path: string) => void }) {
  const [naming, setNaming] = createSignal<{ kind: "rename" | "newFile" | "newFolder"; path: string }>();
  const [filter, setFilter] = createSignal("");
  const [path, setPath] = createSignal("");
  const [files, { refetch }] = createResource(async () => {
    try {
      return { paths: await listTaskFiles(props.taskId) };
    } catch (failure) {
      return { error: String(failure) };
    }
  });
  const matches = createMemo(() => {
    const query = filter().trim().toLowerCase();
    return (files()?.paths ?? []).filter((file) => file.toLowerCase().includes(query));
  });

  const choose = (file: string, keep = false) => {
    setPath(file);
    noteRecentFile(props.taskId, file);
    requestContentTab({ taskId: props.taskId, kind: "file", target: file, temporary: !keep });
  };

  const failed = (title: string, failure: unknown) =>
    pushToast({ tone: "error", title, message: String(failure) });
  const move = async (from: string, toDir: string) => {
    try {
      const next = await moveTaskPath(props.taskId, from, toDir);
      if (path() === from || path().startsWith(`${from}/`)) setPath(next + path().slice(from.length));
      await refetch();
      pushUndoToast(t("files.moved").replace("{path}", () => next), async () => {
        await moveTaskPath(props.taskId, next, from.slice(0, Math.max(0, from.lastIndexOf("/"))));
        await refetch();
      });
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
      if (!folder) choose(made, true);
    } catch (failure) {
      failed(t("files.createFailed"), failure);
    }
  };
  const onAction = async (action: FileAction, target: string, kind: "file" | "dir") => {
    if (action === "mention") {
      props.onInsert(target);
    } else if (action === "open") {
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
        const restorable = await trashTaskPath(props.taskId, target);
        if (path() === target || path().startsWith(`${target}/`)) setPath("");
        await refetch();
        if (restorable) {
          pushUndoToast(t("files.trashed"), async () => {
            await restoreTaskPath(props.taskId, target);
            await refetch();
          });
        } else {
          pushToast({ tone: "success", message: t("files.trashed") });
        }
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
      treeKey={props.taskId} matches={matches()} selected={path()} onSelect={(file) => choose(file)}
      onOpen={(file) => choose(file, true)} onRefresh={() => void reload.run()}
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
