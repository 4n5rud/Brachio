import { Show, createSignal } from "solid-js";
import { t } from "../../i18n";
import { revealPath } from "../../lib/workspace";
import { isMac } from "../../lib/shortcut";
import { pushToast } from "../../lib/toasts";
import { IconButton } from "./IconButton";
import { Menu, MenuItem } from "./Menu";
import { Tooltip } from "./Tooltip";
import "./PathLink.css";

const TAIL_CHARS = 14;
const inApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Splits so the end of the path, which names the thing, survives the ellipsis. */
function splitPath(path: string): [string, string] {
  const cut = Math.max(0, path.length - TAIL_CHARS);
  return [path.slice(0, cut), path.slice(cut)];
}

export async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    pushToast({ tone: "success", message: t("path.copied") });
  } catch (failure) {
    pushToast({ tone: "error", message: String(failure) });
  }
}

/** A file or folder path: mono, one line, middle ellipsis, full path in a tooltip. Click shows it in the file
 * manager (desktop app); the copy button and the context menu copy it. */
export function PathLink(props: { path: string; copyable?: boolean }) {
  const [menu, setMenu] = createSignal<HTMLElement>();
  const reveal = (event: Event) => {
    event.preventDefault();
    event.stopPropagation();
    revealPath(props.path).catch((failure) => pushToast({ tone: "error", message: String(failure) }));
  };
  const parts = () => splitPath(props.path);
  return (
    <span class="path-link">
      <Tooltip label={props.path}>
        {(tip) => (
          <button {...tip} type="button" class="path-link-text" disabled={!inApp} onClick={reveal}
            onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); setMenu(event.currentTarget); }}>
            <span class="path-link-head">{parts()[0]}</span>
            <span class="path-link-tail">{parts()[1]}</span>
          </button>
        )}
      </Tooltip>
      <Show when={props.copyable !== false}>
        <IconButton icon="copy" size="sm" label={t("path.copy")} onClick={(event) => { event.stopPropagation(); void copyPath(props.path); }} />
      </Show>
      <Show when={menu()}>
        {(anchor) => (
          <Menu anchor={anchor()} label={props.path} onClose={() => setMenu(undefined)}>
            <Show when={inApp}>
              <MenuItem onSelect={() => { setMenu(undefined); void revealPath(props.path).catch((failure) => pushToast({ tone: "error", message: String(failure) })); }}>
                {isMac ? t("path.showMac") : t("path.showOther")}
              </MenuItem>
            </Show>
            <MenuItem onSelect={() => { setMenu(undefined); void copyPath(props.path); }}>{t("path.copy")}</MenuItem>
          </Menu>
        )}
      </Show>
    </span>
  );
}
