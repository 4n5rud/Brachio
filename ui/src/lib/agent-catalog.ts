/** What the window knows about each agent; the Rust catalog in brachio-agent holds the same facts. */
export interface AgentEntry {
  /** The id the core stores and sends. */
  coreId: string;
  name: string;
  /** Where the agent's status comes from. */
  status: "hooks" | "osc7501" | "none";
  /** Whether the app answers the agent's permission questions; otherwise the user answers in its terminal. */
  approvalRelay: boolean;
  /** Whether a closed conversation can be continued. */
  resume: boolean;
  /** Key of the mark in the agent icon set; none means the generic bot glyph. */
  icon: "claude" | "codex" | null;
}

export const AGENT_CATALOG = {
  claude: { coreId: "claude_code", name: "Claude Code", status: "hooks", approvalRelay: true, resume: true, icon: "claude" },
  codex: { coreId: "codex", name: "Codex", status: "osc7501", approvalRelay: false, resume: true, icon: "codex" },
  grok: { coreId: "grok", name: "Grok", status: "osc7501", approvalRelay: false, resume: false, icon: null },
} as const satisfies Record<string, AgentEntry>;

/** The short agent names the window uses. */
export type AgentKind = keyof typeof AGENT_CATALOG;

/** The agent names as the core spells them. */
export type CoreAgentKind = (typeof AGENT_CATALOG)[AgentKind]["coreId"];

export const AGENT_KINDS = Object.keys(AGENT_CATALOG) as AgentKind[];

export const agentOfCore = (coreId: CoreAgentKind): AgentKind =>
  AGENT_KINDS.find((kind) => AGENT_CATALOG[kind].coreId === coreId) ?? "claude";
