import { For, Show, createSignal } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { AGENT_CATALOG, AGENT_KINDS } from "../lib/agent-catalog";
import {
  FILTER_STATUSES, clearSidebarFilter, filterActive, sidebarFilter, toggled, updateSidebarFilter, type FilterStatus,
} from "../lib/sidebar-filter";
import { Icon } from "./Icon";
import { IconButton } from "./ui/IconButton";
import { Menu, MenuItem } from "./ui/Menu";
import "./TaskFilter.css";

const STATUS_LABEL: Record<FilterStatus, MessageKey> = {
  needs: "group.needs",
  working: "group.working",
  idle: "filter.idle",
};

/** The sidebar's filter button and its menu of status groups and agents; a row toggles on click. */
export function TaskFilter() {
  const [anchor, setAnchor] = createSignal<HTMLElement>();
  const mark = (on: boolean) => (on ? <Icon name="check" class="icon-sm" /> : <span class="filter-mark" />);
  return (
    <>
      <IconButton class="sidebar-filter" icon="sliders" size="sm" label={t("filter.title")} aria-haspopup="menu"
        aria-expanded={anchor() !== undefined} aria-pressed={filterActive(sidebarFilter())}
        onClick={(event) => setAnchor(anchor() ? undefined : event.currentTarget)} />
      <Show when={anchor()}>
        {(button) => (
          <Menu anchor={button()} label={t("filter.title")} onClose={() => setAnchor(undefined)}>
            <For each={FILTER_STATUSES}>
              {(status) => (
                <MenuItem icon={mark(sidebarFilter().statuses.has(status))}
                  onSelect={() => updateSidebarFilter({ statuses: toggled(sidebarFilter().statuses, status) })}>
                  {t(STATUS_LABEL[status])}
                </MenuItem>
              )}
            </For>
            <div class="filter-divider" role="separator" />
            <For each={AGENT_KINDS}>
              {(agent) => (
                <MenuItem icon={mark(sidebarFilter().agents.has(agent))}
                  onSelect={() => updateSidebarFilter({ agents: toggled(sidebarFilter().agents, agent) })}>
                  {AGENT_CATALOG[agent].name}
                </MenuItem>
              )}
            </For>
            <Show when={filterActive(sidebarFilter())}>
              <div class="filter-divider" role="separator" />
              <MenuItem onSelect={() => { clearSidebarFilter(); setAnchor(undefined); }}>{t("filter.clear")}</MenuItem>
            </Show>
          </Menu>
        )}
      </Show>
    </>
  );
}

/** One line under the sidebar head that says how many tasks the filter hides. */
export function HiddenByFilter(props: { count: number }) {
  return (
    <Show when={props.count > 0}>
      <p class="filter-hidden" role="status">
        <span>{t("filter.hidden").replace("{count}", String(props.count))}</span>
        <button type="button" class="filter-show-all" onClick={clearSidebarFilter}>{t("filter.clear")}</button>
      </p>
    </Show>
  );
}
