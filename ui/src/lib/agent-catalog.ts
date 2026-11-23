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
  /**
   * How a follow-up reaches the agent while it works: `enter` is its own TUI behaviour (type the text and press Enter, which it
   * folds into the running turn); none means no documented way, so the window does not steer it.
   */
  steer: "enter" | null;
  /** Key of the mark in the agent icon set; none means the generic bot glyph. */
  icon: "claude" | "codex" | null;
}

export const AGENT_CATALOG = {
  claude: { coreId: "claude_code", name: "Claude Code", status: "hooks", approvalRelay: true, resume: true, steer: "enter", icon: "claude" },
  codex: { coreId: "codex", name: "Codex", status: "osc7501", approvalRelay: false, resume: true, steer: "enter", icon: "codex" },
  grok: { coreId: "grok", name: "Grok", status: "osc7501", approvalRelay: false, resume: false, steer: null, icon: null },
} as const satisfies Record<string, AgentEntry>;

/** The short agent names the window uses. */
export type AgentKind = keyof typeof AGENT_CATALOG;

/** The agent names as the core spells them. */
export type CoreAgentKind = (typeof AGENT_CATALOG)[AgentKind]["coreId"];

export const AGENT_KINDS = Object.keys(AGENT_CATALOG) as AgentKind[];

export const agentOfCore = (coreId: CoreAgentKind): AgentKind =>
  AGENT_KINDS.find((kind) => AGENT_CATALOG[kind].coreId === coreId) ?? "claude";
