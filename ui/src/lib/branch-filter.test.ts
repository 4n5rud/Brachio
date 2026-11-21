import { describe, expect, it } from "vitest";
import { branchWhere, filterBranches, type BranchChoice } from "./branch-filter";

const list: BranchChoice[] = [
  { name: "feat/relay-inbox", value: "feat/relay-inbox", local: true, remote: null },
  { name: "release/0.4", value: "release/0.4", local: true, remote: "origin" },
  { name: "release/0.3", value: "origin/release/0.3", local: false, remote: "origin" },
  { name: "hotfix/prerelease-fix", value: "hotfix/prerelease-fix", local: true, remote: null },
];

describe("filterBranches", () => {
  it("lists names that start with the text before names that only contain it", () => {
    expect(filterBranches(list, "rel").map((branch) => branch.name)).toEqual([
      "release/0.4", "release/0.3", "feat/relay-inbox", "hotfix/prerelease-fix",
    ]);
  });

  it("ignores case and surrounding spaces, and shows nothing for an unknown name", () => {
    expect(filterBranches(list, " RELEASE/0.3 ").map((branch) => branch.name)).toEqual(["release/0.3"]);
    expect(filterBranches(list, "zzz")).toEqual([]);
  });
});

describe("branchWhere", () => {
  it("says where the branch exists", () => {
    expect(branchWhere(list[1]!, "local", "and")).toBe("local and origin");
    expect(branchWhere(list[2]!, "local", "and")).toBe("origin");
    expect(branchWhere(list[0]!, "local", "and")).toBe("local");
  });
});
