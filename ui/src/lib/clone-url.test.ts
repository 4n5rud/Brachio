import { describe, expect, it } from "vitest";
import { cloneUrlProblem, folderNameForUrl, repoSlug } from "./clone-url";

describe("cloneUrlProblem", () => {
  it("accepts https, ssh and user@host:path URLs", () => {
    for (const url of [
      "https://github.com/owner/repo.git",
      "ssh://git@github.com/owner/repo",
      "git@github.com:owner/repo.git",
    ]) {
      expect(cloneUrlProblem(url)).toBeNull();
    }
  });

  it("names what is wrong with the others", () => {
    expect(cloneUrlProblem("  ")).toBe("empty");
    expect(cloneUrlProblem("file:///etc/passwd")).toBe("scheme");
    expect(cloneUrlProblem("ext::sh -c x")).toBe("format");
    expect(cloneUrlProblem("http://github.com/owner/repo")).toBe("scheme");
    expect(cloneUrlProblem("C:/work/repo")).toBe("scheme");
    expect(cloneUrlProblem("-oProxyCommand=x")).toBe("format");
    expect(cloneUrlProblem("https://github.com")).toBe("format");
  });
});

describe("folderNameForUrl and repoSlug", () => {
  it("reads the folder and owner/repo from a URL", () => {
    expect(folderNameForUrl("https://github.com/owner/repo.git")).toBe("repo");
    expect(folderNameForUrl("git@github.com:owner/my-app.git")).toBe("my-app");
    expect(repoSlug("git@github.com:owner/my-app.git")).toBe("owner/my-app");
    expect(repoSlug("https://github.com/owner/repo/")).toBe("owner/repo");
  });

  it("has no slug without a remote", () => {
    expect(repoSlug(undefined)).toBeNull();
    expect(repoSlug("repo")).toBeNull();
  });
});
