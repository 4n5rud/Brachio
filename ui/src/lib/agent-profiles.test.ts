import { describe, expect, it } from "vitest";
import { compareChoices, compareTitle, profilesFor, type AgentProfileView } from "./agent-profiles";

const profile = (name: string, agent: AgentProfileView["agent"], error: string | null = null): AgentProfileView =>
  ({ name, agent, model: null, args: [], env: [], error });

describe("agent profiles", () => {
  it("lists each agent followed by its usable profiles and skips refused ones", () => {
    const profiles = [profile("fast", "codex"), profile("careful", "claude_code"), profile("bad", "", "not allowed")];
    expect(compareChoices(profiles)).toEqual([
      { agent: "claude" }, { agent: "claude", profile: "careful" },
      { agent: "codex" }, { agent: "codex", profile: "fast" },
      { agent: "grok" },
    ]);
    expect(profilesFor(profiles, "codex").map((item) => item.name)).toEqual(["fast"]);
  });

  it("titles a comparison task with the agent and the profile", () => {
    expect(compareTitle("Fix it", { agent: "codex" }, "Codex")).toBe("Fix it (Codex)");
    expect(compareTitle("Fix it", { agent: "codex", profile: "fast" }, "Codex")).toBe("Fix it (Codex, fast)");
  });
});
