import { t } from "../i18n";
import { Icon } from "./Icon";

/** Sits beside its tab, not inside it, so a button never nests in a button. */
export function TabClose(props: { onClose: () => void; disabled?: boolean; label?: string }) {
  const close = (event: Event) => {
    event.stopPropagation();
    if (!props.disabled) props.onClose();
  };
  return (
    <button class="tab-close" type="button" aria-label={props.label ?? t("task.closeTab")} title={props.label} aria-disabled={props.disabled} onClick={close}>
      <Icon name="x" class="icon-xs" />
    </button>
  );
}
