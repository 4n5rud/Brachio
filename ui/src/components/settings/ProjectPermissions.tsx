import { For, Show, createResource } from "solid-js";
import { t } from "../../i18n";
import { repoPermissions, type ProjectPermissions as Rules } from "../../lib/approvals";
import { SettingRow } from "./SettingRow";
import { SettingsGroup } from "./SettingsGroup";

const summary = (rules: Rules): string => {
  if (rules.error) return t("settings.permissionsInvalid").replace("{error}", rules.error);
  const parts: string[] = [];
  if (rules.allow.length > 0) parts.push(t("settings.permissionsAllow").replace("{tools}", rules.allow.join(", ")));
  if (rules.deny.length > 0) parts.push(t("settings.permissionsDeny").replace("{rules}", rules.deny.join(", ")));
  return parts.length > 0 ? parts.join(". ") : t("settings.permissionsAskAll");
};

function ProjectRules(props: { id: string; name: string }) {
  const [rules] = createResource(() => props.id, (id) =>
    repoPermissions(id).catch((error): Rules => ({ allow: [], deny: [], error: String(error) })));
  return (
    <SettingRow label={props.name} hint={<Show when={rules()}>{(loaded) => summary(loaded())}</Show>}
      error={!!rules()?.error} />
  );
}

/** What each project's `[permissions]` section decides for its agents. */
export function ProjectPermissions(props: { repos: readonly { id: string; name: string }[] }) {
  return (
    <>
      <p class="form-hint">{t("settings.permissionsIntro")}</p>
      <Show when={props.repos.length > 0} fallback={<p class="form-hint">{t("settings.permissionsNone")}</p>}>
        <SettingsGroup>
          <For each={props.repos}>{(repo) => <ProjectRules id={repo.id} name={repo.name} />}</For>
        </SettingsGroup>
      </Show>
    </>
  );
}
