import { For } from "solid-js";
import { t } from "../i18n";
import { statusTone } from "../lib/status";
import type { TaskRow } from "../lib/tasks";
import { PaletteItem } from "./Palette";
import { Dialog } from "./ui/Dialog";
import { StatusGlyph } from "./ui/StatusBadge";

/** Overlay listing the most recently selected tasks while the switch keys are held. */
export function RecentTasksSwitcher(props: {
  tasks: readonly TaskRow[];
  index: number;
  keys: string;
  onPick: (id: string) => void;
  onClose: () => void;
}) {
  return (
    <Dialog title={t("recent.title")} size="quick" bare centered onClose={props.onClose}
      footer={<span class="palette-foot">{t("recent.hint").replace("{keys}", props.keys)}</span>}>
      <div class="palette-list" role="listbox" aria-label={t("recent.title")}>
        <For each={props.tasks}>
          {(task, position) => (
            <PaletteItem on={position() === props.index} lead={<StatusGlyph tone={statusTone(task.status)} />}
              name={task.title} detail={task.branch} onClick={() => props.onPick(task.id)} />
          )}
        </For>
      </div>
    </Dialog>
  );
}
