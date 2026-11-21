import { t } from "../i18n";
import type { AddProjectKind } from "../lib/project-choices";
import { Icon } from "./Icon";
import { MenuItem } from "./ui/Menu";

/** The three ways to add a project, as menu rows. The combobox footer and the sidebar "+" share them. */
export function AddProjectItems(props: { onPick: (kind: AddProjectKind) => void }) {
  return (
    <>
      <MenuItem icon={<Icon name="folderOpen" class="icon-sm" />} onSelect={() => props.onPick("folder")}>
        {t("project.addFolder")}
      </MenuItem>
      <MenuItem icon={<Icon name="download" class="icon-sm" />} onSelect={() => props.onPick("clone")}>
        {t("project.addClone")}
      </MenuItem>
      <MenuItem icon={<Icon name="plus" class="icon-sm" />} onSelect={() => props.onPick("create")}>
        {t("project.addCreate")}
      </MenuItem>
    </>
  );
}
