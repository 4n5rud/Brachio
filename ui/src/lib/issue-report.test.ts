import { describe, expect, it } from "vitest";
import { issueTitle, redactPrivate } from "./issue-report";

describe("redactPrivate", () => {
  it("replaces the known home folder, including one with a space, whatever the slashes or case", () => {
    expect(redactPrivate("failed: c:/users/Kim Lee/repo", "C:\\Users\\Kim Lee")).toBe("failed: ~/repo");
    expect(redactPrivate("in /home/kim lee/x", "/home/kim lee/")).toBe("in ~/x");
  });

  it("replaces home folders on every platform with ~", () => {
    expect(redactPrivate("at C:\\Users\\kim\\repo\\a.ts")).toBe("at ~\\repo\\a.ts");
    expect(redactPrivate("open /Users/kim/repo and /home/lee/x")).toBe("open ~/repo and ~/x");
  });

  it("hides email addresses and secret-shaped strings but keeps ordinary words", () => {
    expect(redactPrivate("author kim.lee+ci@example.co.kr rejected")).toBe("author [email] rejected");
    expect(redactPrivate("token ghp_1a2B3c4D5e6F7g8H9i0J1k2L3m4N5o6P7q8R end"))
      .toBe("token [redacted] end");
    const ordinary = "branch brachio/fix-the-very-long-descriptive-branch-name";
    expect(redactPrivate(ordinary)).toBe(ordinary);
  });
});

describe("issueTitle", () => {
  it("uses the first non-empty line, redacted and capped at 80 characters", () => {
    expect(issueTitle("\n  git failed in /home/kim/repo  \nmore")).toBe("git failed in ~/repo");
    const title = issueTitle("x".repeat(200));
    expect(title).toHaveLength(80);
    expect(title.endsWith("…")).toBe(true);
  });
});
