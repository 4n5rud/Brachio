import { For, Show, createMemo, createSignal } from "solid-js";
import { t } from "../i18n";
import { Dialog } from "./ui/Dialog";
import { Input } from "./ui/Input";

/** Whether `dir` is a folder `from` may move into: not itself, not below itself, not where it already is. */
export function canMoveInto(from: string, dir: string): boolean {
  const parent = from.includes("/") ? from.slice(0, from.lastIndexOf("/")) : "";
  return dir !== parent && dir !== from && !dir.startsWith(`${from}/`);
}

/** Keyboard route for moving an item: pick the destination folder from a filtered list. */
export function FileMoveDialog(props: {
  from: string;
  /** Every folder of the task, without the root. */
  folders: readonly string[];
  onPick: (dir: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = createSignal("");
  const choices = createMemo(() => {
    const needle = query().trim().toLowerCase();
    return ["", ...props.folders]
      .filter((dir) => canMoveInto(props.from, dir) && dir.toLowerCase().includes(needle))
      .slice(0, 200);
  });
  const label = (dir: string) => (dir === "" ? t("files.moveRoot") : dir);
  return (
    <Dialog title={t("files.moveTitle").replace("{name}", () => props.from)} onClose={props.onClose}>
      <Input aria-label={t("files.moveFilter")} placeholder={t("files.moveFilter")} mono value={query()}
        onInput={(event) => setQuery(event.currentTarget.value)}
        onKeyDown={(event) => {
          const first = choices()[0];
          if (event.key === "Enter" && !event.isComposing && first !== undefined) {
            event.preventDefault();
            props.onPick(first);
          }
        }} />
      <ul class="move-choices" aria-label={t("files.moveList")}>
        <For each={choices()}>{(dir) => (
          <li><button type="button" class="tree-row" onClick={() => props.onPick(dir)}>
            <span class="tree-name">{label(dir)}</span>
          </button></li>
        )}</For>
      </ul>
      <Show when={choices().length === 0}><p class="move-empty">{t("files.moveNone")}</p></Show>
    </Dialog>
  );
}
