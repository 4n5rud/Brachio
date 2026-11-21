import { For } from "solid-js";
import { t } from "../i18n";
import { isMac, SHORTCUT_LIST } from "../lib/shortcut";
import { Dialog } from "./ui/Dialog";
import { Kbd } from "./ui/Kbd";
import "./ShortcutList.css";

/** Every shortcut, read from the one list in `shortcut.ts`; either both platforms or only this one. */
export function ShortcutList(props: { platforms: "both" | "current" }) {
  return (
    <div class="shortcut-list" classList={{ "shortcut-list-current": props.platforms === "current" }}>
      {props.platforms === "both" && (
        <div class="shortcut-row shortcut-head">
          <span />
          <span>{t("settings.macos")}</span>
          <span>{t("settings.windows")}</span>
        </div>
      )}
      <For each={SHORTCUT_LIST}>
        {(row) => (
          <div class="shortcut-row">
            <span>{t(row.label)}</span>
            {props.platforms === "both" ? (
              <>
                <Kbd>{row.mac}</Kbd>
                <Kbd>{row.other}</Kbd>
              </>
            ) : (
              <Kbd>{isMac ? row.mac : row.other}</Kbd>
            )}
          </div>
        )}
      </For>
    </div>
  );
}

/** Overlay with the shortcuts of the platform in use. */
export function ShortcutsDialog(props: { onClose: () => void }) {
  return (
    <Dialog title={t("shortcuts.title")} onClose={props.onClose} closeOnScrim escHint>
      <ShortcutList platforms="current" />
    </Dialog>
  );
}
