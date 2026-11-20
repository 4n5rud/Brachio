import { t } from "../i18n";
import { Icon } from "./Icon";
import "./DropVeil.css";

/**
 * Shown while something is dragged over the window. `window` covers it for a folder to add as a
 * project; `terminal` covers one terminal pane for files whose paths go into the agent input.
 */
export function DropVeil(props: { target?: "window" | "terminal" }) {
  const terminal = () => props.target === "terminal";
  return (
    <div class="drop-veil" classList={{ "drop-veil-terminal": terminal() }} role="status">
      <div>
        <Icon name={terminal() ? "download" : "plus"} class="icon drop-veil-icon" />
        <strong>{terminal() ? t("terminal.dropTitle") : t("sidebar.dropTitle")}</strong>
        <span>{terminal() ? t("terminal.dropNote") : t("sidebar.dropBody")}</span>
      </div>
    </div>
  );
}
