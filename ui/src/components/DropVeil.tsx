import { t } from "../i18n";
import { Icon } from "./Icon";
import "./DropVeil.css";

/** Covers the window while a folder is dragged over it: dropping adds the folder as a project. */
export function DropVeil() {
  return (
    <div class="drop-veil" role="status">
      <div>
        <Icon name="plus" class="icon drop-veil-icon" />
        <strong>{t("sidebar.dropTitle")}</strong>
        <span>{t("sidebar.dropBody")}</span>
      </div>
    </div>
  );
}
