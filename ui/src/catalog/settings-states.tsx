import type { JSX } from "solid-js";
import { sampleTasks } from "../lib/sample";
import { AgentCard, type AccountRowData, type AccountState, type AgentCardProps } from "../components/settings/AgentCard";
import { AgentList } from "../components/settings/AgentSettings";
import { SettingsPanel, type SettingsPage, type UpdateSettingsProps } from "../components/settings/SettingsPanel";
import { Button } from "../components/ui/Button";
import { t } from "../i18n";
import { Cell } from "./changes-states";

const NOW = Date.now();
const none = () => {};
const ARCHIVED = sampleTasks(NOW).slice(0, 2).map((task, index) => ({ ...task, repoId: "app", archivedAt: NOW - (index + 1) * 3_600_000 }));
const REPOS = [{ id: "app", name: "brachio" }];

const signedIn = (account: string): AccountState => ({ state: "signedIn", account });
const SYSTEM: AccountRowData = { id: null, name: "System default", system: true, isDefault: true, state: signedIn("me@example.com") };
const WORK: AccountRowData = { id: "work", name: "Work", system: false, isDefault: false, state: signedIn("work@example.com") };
const TEAM: AccountRowData = { id: "team", name: "Team", system: false, isDefault: false, state: { state: "signedOut" } };
const handlers = { onRefresh: none, onAdd: async () => true, onRename: async () => true, onMakeDefault: none, onSignIn: none, onSignOut: none, onRemove: none, onCancelSignIn: none };
const CLAUDE: AgentCardProps = { kind: "claude_code", version: "2.1.330", path: "/usr/local/bin/claude", account: signedIn("me@example.com"),
  accounts: [SYSTEM, WORK, TEAM], onClaudeUsage: none, ...handlers };
const CODEX: AgentCardProps = { kind: "codex", version: "0.171.0", path: "/Users/you/.local/bin/codex", account: signedIn("work@example.com"),
  accounts: [{ ...SYSTEM, isDefault: false }, { ...WORK, isDefault: true }], usageAccount: "Work", ...handlers,
  usage: [{ label: "5 h window", percent: 62, reset: "Fri 17:40" }, { label: "Weekly window", percent: 18, reset: "Mon 09:00" }] };
const updates = (over: Partial<UpdateSettingsProps>): UpdateSettingsProps =>
  ({ enabled: true, status: "idle", version: null, current: "0.4.0", onEnabled: none, onCheck: none, onDownload: none, ...over });

function Frame(props: { page: SettingsPage; agents?: JSX.Element; updates?: Partial<UpdateSettingsProps>; drafts?: { fontSize?: string }; empty?: boolean }) {
  return (
    <div class="modal-flush catalog-settings">
      <div class="modal-head"><h2>Settings</h2>
        <div class="modal-head-end"><Button size="sm" onClick={none}>{t("workspace.close")}</Button></div></div>
      <div class="modal-body">
        <SettingsPanel page={props.page} mode="light" colorTheme="sand" onMode={none} onColorTheme={none} notifications onNotifications={none}
          updates={updates(props.updates ?? {})} agents={props.agents} terminalDrafts={props.drafts}
          archived={props.empty ? [] : ARCHIVED} repos={REPOS} now={NOW} onRestore={async () => {}} onDelete={async () => {}} onReportProblem={none} />
      </div>
    </div>
  );
}

const agentList = (...list: AgentCardProps[]) => (
  <AgentList agents={list.map((card) => ({ kind: card.kind, path: card.path, version: card.version }))} onRefresh={none}
    card={(agent) => <AgentCard {...list.find((card) => card.kind === agent.kind) as AgentCardProps} />} />
);

/** Every Settings page in the states the design shows. */
export function SettingsStates() {
  return (
    <div class="catalog-cells">
      <Cell cap="General, language menu opens from the button" width={820}><Frame page="general" /></Cell>
      <Cell cap="Appearance, Sand light" width={820}><Frame page="appearance" /></Cell>
      <Cell cap="Terminal" width={820}><Frame page="terminal" /></Cell>
      <Cell cap="Terminal, a value out of range" width={820}><Frame page="terminal" drafts={{ fontSize: "30" }} /></Cell>
      <Cell cap="Agents, signed in with usage" width={820}><Frame page="agents" agents={agentList(CLAUDE, CODEX)} /></Cell>
      <Cell cap="Agents, checking" width={820}>
        <Frame page="agents" agents={agentList(
          { ...CLAUDE, account: { state: "checking" }, accounts: CLAUDE.accounts?.map((row) => ({ ...row, state: { state: "checking" } })) },
          { ...CODEX, account: { state: "checking" }, accounts: CODEX.accounts?.map((row) => ({ ...row, state: { state: "checking" } })), usage: undefined })} />
      </Cell>
      <Cell cap="Agents, signing in to Codex in the browser" width={820}>
        <Frame page="agents" agents={agentList(CLAUDE, { ...CODEX, account: { state: "signedOut" }, accounts: [{ ...SYSTEM, isDefault: false }, { ...WORK, isDefault: true, state: { state: "signedOut" }, signingIn: true }], usage: undefined, busy: true })} />
      </Cell>
      <Cell cap="Agents, an account signed out and one that failed to read" width={820}>
        <Frame page="agents" agents={agentList(
          { ...CLAUDE, accounts: [SYSTEM, TEAM, { ...WORK, state: { state: "error", message: "The CLI did not answer." } }] }, CODEX)} />
      </Cell>
      <Cell cap="Agents, none found" width={820}><Frame page="agents" agents={<AgentList agents={[]} onRefresh={none} card={() => null} />} /></Cell>
      <Cell cap="Agents, the list could not be read" width={820}>
        <Frame page="agents" agents={<AgentList agents={[]} error="PATH lookup timed out." onRefresh={none} card={() => null} />} />
      </Cell>
      <Cell cap="Notifications" width={820}><Frame page="notifications" /></Cell>
      <Cell cap="Shortcuts, macOS and Windows" width={820}><Frame page="shortcuts" /></Cell>
      <Cell cap="Updates, a new version" width={820}><Frame page="updates" updates={{ version: "0.5.0" }} /></Cell>
      <Cell cap="Updates, checking" width={820}><Frame page="updates" updates={{ status: "checking" }} /></Cell>
      <Cell cap="Updates, check failed" width={820}><Frame page="updates" updates={{ status: "unavailable" }} /></Cell>
      <Cell cap="Archived tasks" width={820}><Frame page="archived" /></Cell>
      <Cell cap="Archived tasks, empty" width={820}><Frame page="archived" empty /></Cell>
    </div>
  );
}
