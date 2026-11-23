import { describe, expect, it } from "vitest";
import { describeActivity } from "./task-activity";

const entry = (kind: string, detail: string | null) => ({ id: 1, kind, detail, at: 0 });

describe("timeline entries", () => {
  it("shows agent status changes with the tone of that state", () => {
    expect(describeActivity(entry("agent_status", "working")).tone).toBe("working");
    expect(describeActivity(entry("agent_status", "idle")).tone).toBe("ready");
    expect(describeActivity(entry("agent_status", "blocked")).tone).toBe("blocked");
  });

  it("shows the restore point number and tells a restore from a new point", () => {
    const created = describeActivity(entry("restore_point", "created #4"));
    const restored = describeActivity(entry("restore_point", "restored #4"));
    expect(created.detail).toBe("#4");
    expect(restored.detail).toBe("#4");
    expect(restored.label).not.toBe(created.label);
  });

  it("shows pull request events with their number and a failing tone for failed checks", () => {
    expect(describeActivity(entry("pr_event", "merged #7"))).toMatchObject({ detail: "#7", tone: "done" });
    expect(describeActivity(entry("pr_event", "checks_failed #7")).tone).toBe("error");
  });

  it("keeps an unknown pull request event instead of hiding it", () => {
    expect(describeActivity(entry("pr_event", "something #1")).detail).toBe("something #1");
  });
});
