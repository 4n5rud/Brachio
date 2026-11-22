import { For, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { startFileDrag } from "../lib/file-drag";
import { osDropDir, treeDirAt } from "../lib/tree-drop";
import { terminalAt, type TerminalDrop } from "../lib/terminal-drop";
import { Icon } from "./Icon";
import { FileMoveDialog, canMoveInto } from "./FileMoveDialog";
import { Menu, MenuItem } from "./ui/Menu";
import "./FileTree.css";

interface Node { name: string; path: string; dirs: Map<string, Node>; files: string[] }
interface Row { kind: "dir" | "file"; path: string; name: string; depth: number; open: boolean }

// Open folders live outside the component so they survive tab switches.
const openByTask = new Map<string, Set<string>>();

function build(paths: readonly string[]): Node {
  const root: Node = { name: "", path: "", dirs: new Map(), files: [] };
  for (const path of paths) {
    const parts = path.split("/");
    let node = root;
    for (let i = 0; i < parts.length - 1; i++) {
      const name = parts[i] ?? "";
      let next = node.dirs.get(name);
      if (!next) node.dirs.set(name, (next = { name, path: parts.slice(0, i + 1).join("/"), dirs: new Map(), files: [] }));
      node = next;
    }
    node.files.push(path);
  }
  return root;
}

const byName = (a: string, b: string) => a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" });
const baseName = (path: string) => path.slice(path.lastIndexOf("/") + 1);
const parentOf = (path: string) => (path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "");
/** How long a dragged item must rest on a closed folder before it opens. */
const HOVER_OPEN_MS = 600;

function Highlight(props: { text: string; query: string }) {
  const at = () => (props.query ? props.text.toLowerCase().indexOf(props.query.toLowerCase()) : -1);
  return (
    <Show when={at() >= 0} fallback={props.text}>
      {props.text.slice(0, at())}<mark>{props.text.slice(at(), at() + props.query.length)}</mark>{props.text.slice(at() + props.query.length)}
    </Show>
  );
}

/** Folder tree of the task's files; a search shows the matching files flat, with their folder. */
export function FileTree(props: {
  paths: readonly string[];
  query: string;
  treeKey: string;
  selected: string;
  onSelect: (path: string) => void;
  limit: number;
  /** Moves an item into a folder ("" is the root); the caller reports failures. */
  onMove?: (from: string, toDir: string) => void;
  /** An item was dropped on a terminal. */
  onTerminalDrop?: (path: string, terminal: TerminalDrop) => void;
}) {
  const [open, setOpen] = createSignal(openByTask.get(props.treeKey) ?? new Set<string>());
  const [focus, setFocus] = createSignal("");
  const toggle = (path: string, force?: boolean) => {
    const next = new Set(open());
    if (force ?? !next.has(path)) next.add(path); else next.delete(path);
    openByTask.set(props.treeKey, next);
    setOpen(next);
  };
  const root = createMemo(() => build(props.paths));
  const rows = createMemo<Row[]>(() => {
    const out: Row[] = [];
    const walk = (node: Node, depth: number) => {
      for (const dir of [...node.dirs.values()].sort((a, b) => byName(a.name, b.name))) {
        const isOpen = open().has(dir.path);
        out.push({ kind: "dir", path: dir.path, name: dir.name, depth, open: isOpen });
        if (isOpen) walk(dir, depth + 1);
      }
      for (const file of [...node.files].sort((a, b) => byName(baseName(a), baseName(b)))) {
        out.push({ kind: "file", path: file, name: baseName(file), depth, open: false });
      }
    };
    walk(root(), 0);
    return out;
  });
  const searching = () => props.query !== "";
  let treeEl: HTMLDivElement | undefined;
  const [dragTarget, setDragTarget] = createSignal<string>();
  const [menu, setMenu] = createSignal<{ row: Row; anchor: HTMLElement }>();
  const [moving, setMoving] = createSignal<string>();
  const hoverDir = () => dragTarget() ?? osDropDir();
  const folders = createMemo(() => {
    const out: string[] = [];
    const walk = (node: Node) => {
      for (const dir of node.dirs.values()) { out.push(dir.path); walk(dir); }
    };
    walk(root());
    return out.sort(byName);
  });

  // A folder opens after a dragged item rests on it, so the destination can be a nested folder.
  createEffect(() => {
    const dir = hoverDir();
    if (!dir || open().has(dir)) return;
    const timer = setTimeout(() => toggle(dir, true), HOVER_OPEN_MS);
    onCleanup(() => clearTimeout(timer));
  });

  let overTerminal: Element | undefined;
  const markTerminal = (host?: Element) => {
    if (host === overTerminal) return;
    overTerminal?.removeAttribute("data-drop-over");
    host?.setAttribute("data-drop-over", "");
    overTerminal = host;
  };
  const beginDrag = (event: PointerEvent, row: Row) => {
    startFileDrag(event, {
      label: row.name,
      onMove: (x, y) => {
        const terminal = terminalAt(x, y);
        markTerminal(terminal?.host);
        const dir = terminal ? undefined : treeDirAt(x, y, treeEl);
        setDragTarget(dir !== undefined && canMoveInto(row.path, dir) ? dir : undefined);
      },
      onDrop: (x, y) => {
        const terminal = terminalAt(x, y);
        const dir = dragTarget();
        if (terminal) props.onTerminalDrop?.(row.path, terminal);
        else if (dir !== undefined) props.onMove?.(row.path, dir);
      },
      onEnd: () => { markTerminal(undefined); setDragTarget(undefined); },
    });
  };

  const move = (event: KeyboardEvent) => {
    const list = rows();
    const at = Math.max(list.findIndex((row) => row.path === (focus() || props.selected)), 0);
    const row = list[at];
    if (!row) return;
    const go = (index: number) => {
      const target = list[Math.min(Math.max(index, 0), list.length - 1)];
      if (!target) return;
      setFocus(target.path);
      (event.currentTarget as HTMLElement).querySelector<HTMLElement>(`[data-path="${CSS.escape(target.path)}"]`)?.focus();
    };
    const parent = () => {
      for (let i = at - 1; i >= 0; i--) if (list[i]?.kind === "dir" && (list[i]?.depth ?? 0) < row.depth) return i;
      return -1;
    };
    switch (event.key) {
      case "ArrowDown": go(at + 1); break;
      case "ArrowUp": go(at - 1); break;
      case "Home": go(0); break;
      case "End": go(list.length - 1); break;
      case "ArrowRight":
        if (row.kind === "dir") { if (!row.open) toggle(row.path, true); else go(at + 1); }
        break;
      case "ArrowLeft":
        if (row.kind === "dir" && row.open) toggle(row.path, false); else if (parent() >= 0) go(parent());
        break;
      case "Enter": case " ":
        if (row.kind === "dir") toggle(row.path); else props.onSelect(row.path);
        break;
      default: return;
    }
    event.preventDefault();
  };
  const tabbable = (row: Row) => row.path === (focus() || props.selected || rows()[0]?.path);

  return (
    <Show when={!searching()} fallback={
      <ul class="file-results" aria-label={t("files.results")}>
        <For each={props.paths.slice(0, props.limit)}>{(file) => (
          <li>
            <button type="button" class="tree-row" data-current={file === props.selected || undefined} title={file}
              onClick={() => props.onSelect(file)}>
              <Icon name="file" class="icon tree-icon" />
              <span class="tree-name"><Highlight text={baseName(file)} query={props.query} />
                <Show when={file.includes("/")}>
                  <span class="tree-dir"><Highlight text={file.slice(0, file.lastIndexOf("/"))} query={props.query} /></span>
                </Show>
              </span>
            </button>
          </li>
        )}</For>
      </ul>
    }>
      <div ref={treeEl} class="file-tree" role="tree" aria-label={t("files.tree")} onKeyDown={move}
        data-drop-dir="" classList={{ "drop-root": hoverDir() === "" }}>
        <For each={rows()}>{(row) => (
          <div role="treeitem" class="tree-row" classList={{ dir: row.kind === "dir", "drop-target": row.kind === "dir" && hoverDir() === row.path }}
            data-drop-dir={row.kind === "dir" ? row.path : parentOf(row.path)} data-current={row.path === props.selected || undefined}
            data-path={row.path} tabindex={tabbable(row) ? 0 : -1} title={row.path}
            aria-level={row.depth + 1} aria-expanded={row.kind === "dir" ? row.open : undefined}
            aria-selected={row.kind === "file" ? row.path === props.selected : undefined}
            style={{ "--depth": row.depth }}
            onFocus={() => setFocus(row.path)}
            onPointerDown={(event) => beginDrag(event, row)}
            onContextMenu={(event) => { event.preventDefault(); setMenu({ row, anchor: event.currentTarget }); }}
            onClick={() => (row.kind === "dir" ? toggle(row.path) : props.onSelect(row.path))}>
            <Show when={row.kind === "dir"} fallback={<span class="tree-chevron" />}>
              <Icon name={row.open ? "chevronDown" : "chevronRight"} class="icon tree-chevron" />
            </Show>
            <Icon name={row.kind === "dir" ? (row.open ? "folderOpen" : "folder") : "file"} class="icon tree-icon" />
            <span class="tree-name">{row.name}</span>
          </div>
        )}</For>
      </div>
      <Show when={menu()}>
        {(current) => (
          <Menu anchor={current().anchor} label={t("files.menu")} onClose={() => setMenu(undefined)}>
            <MenuItem disabled={!props.onMove} onSelect={() => { setMoving(current().row.path); setMenu(undefined); }}>
              {t("files.move")}
            </MenuItem>
          </Menu>
        )}
      </Show>
      <Show when={moving()}>
        {(from) => (
          <FileMoveDialog from={from()} folders={folders()} onClose={() => setMoving(undefined)}
            onPick={(dir) => { setMoving(undefined); props.onMove?.(from(), dir); }} />
        )}
      </Show>
    </Show>
  );
}
