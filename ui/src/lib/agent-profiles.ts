import { invoke } from "@tauri-apps/api/core";
import type { AgentKind as CoreAgent } from "./terminal";
import { AGENT_KINDS, agentOfCore } from "./agent-catalog";
import type { AgentKind } from "./tasks";

/** A profile from the repository's `brachio.toml`, or the reason it cannot be used. */
export interface AgentProfileView {
  name: string;
  /** Empty when the profile was refused. */
  agent: CoreAgent | "";
  model: string | null;
  args: string[];
  /** Names only. */
  env: string[];
  error: string | null;
}

export const repoProfiles = (repoId: string) => invoke<AgentProfileView[]>("repo_profiles", { repoId });
export const joinCompareGroup = (taskId: string, group: string) =>
  invoke<string>("task_join_compare_group", { taskId, group });

/** One run of a comparison: an agent, optionally with one of its profiles. */
export interface CompareTarget {
  agent: AgentKind;
  profile?: string;
}

/** The usable profiles of one agent. */
export const profilesFor = (profiles: readonly AgentProfileView[], agent: AgentKind) =>
  profiles.filter((profile) => profile.error === null && profile.agent !== "" && agentOfCore(profile.agent) === agent);

/** Short text for a profile choice: its name and model. */
export const profileLabel = (profile: AgentProfileView) =>
  profile.model ? `${profile.name} (${profile.model})` : profile.name;

/** The agents and profiles a comparison can run, in display order. */
export const compareChoices = (profiles: readonly AgentProfileView[]): CompareTarget[] => {
  const choices: CompareTarget[] = [];
  for (const agent of AGENT_KINDS) {
    choices.push({ agent });
    for (const profile of profilesFor(profiles, agent)) choices.push({ agent, profile: profile.name });
  }
  return choices;
};

/** Title for a task made for `target` from `title`. */
export const compareTitle = (title: string, target: CompareTarget, agentName: string) =>
  `${title} (${target.profile ? `${agentName}, ${target.profile}` : agentName})`;
