import { For, Show, createSignal } from "solid-js";
import { open as pickFiles } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { Icon } from "./Icon";
import { IconButton } from "./ui/IconButton";
import { Menu, MenuItem, MenuSubmenu } from "./ui/Menu";

export interface TailChoice {
  key: string;
  label: string;
}

const IMAGE_FILTER = [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }];

/** Menu that picks files, images or another terminal's last lines to put in a prompt as a draft. */
export function AttachButton(props: {
  /** Where the file pickers start. */
  defaultPath?: string;
  tails: () => readonly TailChoice[];
  onPaths: (paths: string[], kind: "files" | "images") => void;
  onTail: (key: string) => void;
  size?: "md" | "sm";
  class?: string;
}) {
  const [anchor, setAnchor] = createSignal<HTMLElement>();
  const choose = async (kind: "files" | "images") => {
    setAnchor(undefined);
    const picked = await pickFiles({
      multiple: true, directory: false, defaultPath: props.defaultPath,
      filters: kind === "images" ? IMAGE_FILTER : undefined,
    });
    if (picked && picked.length > 0) props.onPaths(Array.isArray(picked) ? picked : [picked], kind);
  };
  return (
    <>
      <IconButton icon="file" size={props.size ?? "sm"} class={props.class} label={t("attach.title")} aria-haspopup="menu"
        aria-expanded={anchor() !== undefined}
        onClick={(event) => setAnchor(anchor() ? undefined : event.currentTarget)} />
      <Show when={anchor()}>
        {(at) => (
          <Menu anchor={at()} label={t("attach.title")} onClose={() => setAnchor(undefined)}>
            <MenuItem icon={<Icon name="file" class="icon-sm" />} onSelect={() => void choose("files")}>{t("attach.files")}</MenuItem>
            <MenuItem icon={<Icon name="download" class="icon-sm" />} onSelect={() => void choose("images")}>{t("attach.images")}</MenuItem>
            <MenuSubmenu icon={<Icon name="terminal" class="icon-sm" />} label={t("attach.terminal")}>
              <For each={props.tails()} fallback={<MenuItem disabled onSelect={() => {}}>{t("attach.noTerminal")}</MenuItem>}>
                {(choice) => <MenuItem onSelect={() => { setAnchor(undefined); props.onTail(choice.key); }}>{choice.label}</MenuItem>}
              </For>
            </MenuSubmenu>
          </Menu>
        )}
      </Show>
    </>
  );
}
