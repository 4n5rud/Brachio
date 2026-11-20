import { expect, test } from "vitest";
import { classifyGitFailure, gitOutput } from "./git-failure";

test("keeps only what Git printed, even when the commit message holds a colon", () => {
  const message = "git commit -m feat(auth): limit failed with status 1: husky - pre-commit hook failed";
  expect(gitOutput(message)).toBe("husky - pre-commit hook failed");
  expect(classifyGitFailure("commit", message).kind).toBe("hook");
});

test("tells a rejected push from a denied one", () => {
  const rejected = "git push failed with status 1: ! [rejected] a -> a (fetch first)\nerror: failed to push some refs";
  expect(classifyGitFailure("push", rejected).kind).toBe("rejected");
  const denied = "git push failed with status 128: remote: Permission to 4n5rud/brachio.git denied to bot.\nfatal: unable to access: The requested URL returned error: 403";
  expect(classifyGitFailure("push", denied)).toMatchObject({ kind: "denied", repository: "4n5rud/brachio" });
});

test("finds the pull request that already exists", () => {
  const message = 'a pull request for branch "x" into branch "main" already exists:\nhttps://github.com/4n5rud/brachio/pull/242';
  expect(classifyGitFailure("createPr", message)).toMatchObject({
    kind: "exists",
    pullRequestUrl: "https://github.com/4n5rud/brachio/pull/242",
    pullRequestNumber: 242,
  });
  expect(classifyGitFailure("commit", "git commit failed with status 1: nothing to commit").kind).toBe("other");
});
