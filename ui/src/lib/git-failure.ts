export type GitFailureKind = "hook" | "rejected" | "denied" | "exists" | "other";

export interface GitFailure {
  kind: GitFailureKind;
  /** What Git or gh printed, without the command line, for the output block. */
  output: string;
  /** Repository named by a push denial, such as `owner/name`. */
  repository?: string;
  /** Address of the pull request that already exists. */
  pullRequestUrl?: string;
  pullRequestNumber?: number;
}

const MAX_OUTPUT = 1200;

/** The error text of `task_git_action` is `git <args> failed with status N: <stderr>`; the arguments may hold the commit message. */
export function gitOutput(message: string): string {
  const printed = /failed with status \S+: ([\s\S]*)$/.exec(message)?.[1] ?? message;
  const trimmed = printed.trim();
  return trimmed.length > MAX_OUTPUT ? `…${trimmed.slice(-MAX_OUTPUT)}` : trimmed;
}

/** Maps the text of a failed Git action to one of the states the design draws; anything else stays `other`. */
export function classifyGitFailure(action: "commit" | "push" | "createPr" | "other", message: string): GitFailure {
  const output = gitOutput(message);
  if (action === "createPr") {
    if (/already exists/i.test(message)) {
      const url = /https:\/\/\S+\/pull\/(\d+)/.exec(message);
      return {
        kind: "exists",
        output,
        pullRequestUrl: url?.[0],
        pullRequestNumber: url ? Number(url[1]) : undefined,
      };
    }
  } else if (action === "push") {
    const denied = /Permission to (\S+?)(?:\.git)? denied|returned error: 40[13]|Authentication failed|could not read Username|Permission denied \(publickey\)/i
      .exec(message);
    if (denied) return { kind: "denied", output, repository: denied[1] };
    if (/\[rejected\]|non-fast-forward|fetch first|failed to push some refs/i.test(message)) {
      return { kind: "rejected", output };
    }
  } else if (action === "commit" && /hook/i.test(message)) {
    return { kind: "hook", output };
  }
  return { kind: "other", output };
}
