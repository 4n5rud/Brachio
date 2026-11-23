import { expect, test } from "vitest";
import { AGENT_CATALOG, AGENT_KINDS, agentOfCore } from "./agent-catalog";
import { AGENT_NAME } from "./tasks";
import { CORE_AGENT } from "./terminal";

test("each agent maps to its core id and back", () => {
  expect(AGENT_KINDS).toEqual(["claude", "codex", "grok"]);
  expect(AGENT_KINDS.map((kind) => CORE_AGENT[kind])).toEqual(["claude_code", "codex", "grok"]);
  for (const kind of AGENT_KINDS) expect(agentOfCore(CORE_AGENT[kind])).toBe(kind);
  expect(AGENT_NAME.grok).toBe("Grok");
});

test("only Claude Code relays approvals into the app and Grok reports through OSC 7501 only", () => {
  expect(AGENT_KINDS.filter((kind) => AGENT_CATALOG[kind].approvalRelay)).toEqual(["claude"]);
  expect(AGENT_CATALOG.grok.status).toBe("osc7501");
  expect(AGENT_CATALOG.grok.resume).toBe(false);
});
