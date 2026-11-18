import { t } from "../i18n";
import { Icon } from "./Icon";

/** Bottom bar. Resource numbers come from the core once M2 wires them. */
export function StatusBar(props: { sessions: number }) {
  return (
    <footer class="statusbar">
      <span>
        <i class="dot" />
        {t("statusbar.core")}
      </span>
      <span>
        <Icon name="terminal" class="icon-sm" />
        {t("statusbar.sessions")} <b>{props.sessions}</b>
      </span>
      <span class="statusbar-end">
        <Icon name="memory" class="icon-sm" />
        {t("statusbar.memory")} <b>–</b>
        <Icon name="cpu" class="icon-sm" />
        {t("statusbar.cpu")} <b>–</b>
      </span>
    </footer>
  );
}
