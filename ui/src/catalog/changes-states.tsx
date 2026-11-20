import { createSignal, type JSX } from "solid-js";
import { t } from "../i18n";
import { ChangedFiles, type ChangesData } from "../components/ChangedFiles";
import { DiffReview } from "../components/DiffReview";
import { FileRow } from "../components/FileRow";
import { InspectorNote } from "../components/InspectorNote";
import { InspectorSection } from "../components/InspectorSection";
import { MergeCheckView } from "../components/TaskMergePreview";
import { PullRequestFeedbackView, type FeedbackItem } from "../components/PullRequestFeedback";
import { PullRequestPanel } from "../components/PullRequestStatus";
import { TaskGitActions } from "../components/TaskGitActions";
import { Button } from "../components/ui/Button";
import { confirmDialog } from "../components/ui/ConfirmDialog";
import { diffFiles } from "../lib/diff";
import type { PrStatusReport, PullRequestSnapshot } from "../lib/pr-status";
import type { GitRunner } from "../lib/task-git-runner";

const NOW = Date.now();
const none = () => Promise.resolve(true);
const runner: GitRunner = { busy: () => false, run: () => Promise.resolve(undefined) };

const PATCH = [
  "diff --git a/crates/brachio-auth/src/limiter.rs b/crates/brachio-auth/src/limiter.rs",
  "new file mode 100644",
  "--- /dev/null",
  "+++ b/crates/brachio-auth/src/limiter.rs",
  "@@ -0,0 +1,2 @@",
  "+pub struct Limiter;",
  "+impl Limiter {}",
  "diff --git a/crates/brachio-auth/src/login.rs b/crates/brachio-auth/src/login.rs",
  "--- a/crates/brachio-auth/src/login.rs",
  "+++ b/crates/brachio-auth/src/login.rs",
  "@@ -12,5 +12,6 @@ pub async fn login(",
  "     let ip = client_ip(&req);",
  "-    let user = find_user(&body.email)?;",
  "+    if !limiter.allow(ip) {",
  "+        return Err(TooMany::retry_after(60));",
  "+    }",
  "     verify(&user, &body.password)?;",
  "diff --git a/crates/brachio-auth/src/old_guard.rs b/crates/brachio-auth/src/old_guard.rs",
  "deleted file mode 100644",
  "--- a/crates/brachio-auth/src/old_guard.rs",
  "+++ /dev/null",
  "@@ -1,2 +0,0 @@",
  "-pub fn guard() {}",
  "-pub fn unguard() {}",
  "diff --git a/old.md b/docs/limits.md",
  "similarity index 80%",
  "rename from old.md",
  "rename to docs/limits.md",
  "--- a/old.md",
  "+++ b/docs/limits.md",
  "@@ -1 +1 @@",
  "-Five tries",
  "+Five tries a minute",
].join("\n");

const files = diffFiles(PATCH);
const base: ChangesData = { patch: PATCH, staged: [], untracked: [], conflicts: [], truncated: false };
const changed = (data: Partial<ChangesData> = {}, extra: Partial<Parameters<typeof ChangedFiles>[0]> = {}) => (
  <ChangedFiles additions={131} deletions={56} loading={false} data={{ ...base, ...data }} files={files} busy={false}
    split={false} onSplit={() => {}} onRefresh={() => {}} onStage={() => {}} onDraft={none} {...extra} />
);
const empty: ChangesData = { patch: "", staged: [], untracked: [], conflicts: [], truncated: false };

const snapshot = (over: Partial<PullRequestSnapshot> = {}): PullRequestSnapshot => ({
  repository: "4n5rud/brachio", number: 239, url: "https://github.com/4n5rud/brachio/pull/239", state: "open",
  draft: false, baseBranch: "main", headBranch: "pr-checks-c04d", headSha: "c04d91e7a1b2c3d4",
  checks: { overall: "failure", passing: 6, failing: 1, pending: 1, skipped: 0 },
  failedChecks: [{ name: "ci / test (windows-latest)", url: "https://github.com/4n5rud/brachio/actions/runs/1" }],
  review: "changes_requested", conflict: "conflicting", ...over,
});
const report = (over: Partial<PrStatusReport> = {}): PrStatusReport => ({
  snapshot: snapshot(), checkedAtMs: NOW - 60_000, noPullRequest: false, error: null, retryInSeconds: null,
  localHead: "c04d91e7a1b2c3d4", headMatchesLocal: true, ...over,
});
const panel = (value?: PrStatusReport, failure?: string) => (
  <PullRequestPanel report={value} failure={failure} busy={false} now={NOW} onRefresh={() => {}} />
);

const feedbackItems: FeedbackItem[] = [
  { kind: "check", label: "ci / test (windows-latest)", url: "https://github.com", text: "" },
  { kind: "comment", label: "Reviewer login.rs:14", body: "Read the window from config.", url: "https://github.com", text: "" },
];
const feedback = (over: Partial<Parameters<typeof PullRequestFeedbackView>[0]> = {}) => (
  <PullRequestFeedbackView items={feedbackItems} loaded busy={false} error="" inlineError={null} onLoad={() => {}}
    onInsert={() => {}} {...over} />
);

const git = (initial: Parameters<typeof TaskGitActions>[0]["initial"] = {}, staged: string[] = ["crates/brachio-auth/src/login.rs"],
  conflicts: string[] = []) => (
  <TaskGitActions taskId="catalog" branch="brachio/rate-limit-7f2a" runner={runner} files={files} staged={staged}
    untracked={[]} conflicts={conflicts} onDraft={none} initial={initial} />
);

const REJECTED = "! [rejected]        brachio/rate-limit-7f2a -> brachio/rate-limit-7f2a (fetch first)\nerror: failed to push some refs";
const DENIED = "remote: Permission to 4n5rud/brachio.git denied to jmk-bot.\nfatal: unable to access 'https://github.com/4n5rud/brachio.git/': The requested URL returned error: 403";

function Cell(props: { cap: string; width?: number; children: JSX.Element }) {
  return (
    <div class="catalog-cell">
      <span class="catalog-cap">{props.cap}</span>
      <div class="catalog-inspector" style={{ width: `${props.width ?? 360}px` }}>{props.children}</div>
    </div>
  );
}

/** The Changes tab in every state it can show, built from the same components the inspector uses. */
export function ChangesStates() {
  const [opened, setOpened] = createSignal(false);
  const checkDraft = async () => {
    setOpened(true);
    await confirmDialog({
      title: t("draft.title"),
      body: [
        t("draft.target").replace("{agent}", "Claude Code").replace("{task}", "Add rate limit to the login endpoint"),
        t("draft.review"),
        "[crates/brachio-auth/src/login.rs:14 · after] Read the window from config.",
      ].join("\n\n"),
      confirmLabel: t("draft.insert"),
      cancelLabel: t("workspace.close"),
      tone: "primary",
    });
    setOpened(false);
  };
  return (
    <div class="catalog-cells">
      <Cell cap="Loading">{[<ChangedFiles loading files={[]} busy={false} split={false} onSplit={() => {}} onRefresh={() => {}}
        onStage={() => {}} onDraft={none} />, panel()]}</Cell>
      <Cell cap="Nothing changed yet">{[changed(empty, { additions: 0, deletions: 0 }), panel(report({ snapshot: null, noPullRequest: true }))]}</Cell>
      <Cell cap="Files: added, modified, deleted, renamed">{changed()}</Cell>
      <Cell cap="Line totals unavailable">
        <ChangedFiles additions={undefined} deletions={undefined} loading={false} files={[]} busy={false} split={false}
          onSplit={() => {}} onRefresh={() => {}} onStage={() => {}} onDraft={none} />
      </Cell>
      <Cell cap="Large diff cut">{changed({ truncated: true, patch: PATCH.split("\n").slice(0, 7).join("\n") })}</Cell>
      <Cell cap="Changes could not be read">
        <ChangedFiles loading={false} error="git status failed with status 128: fatal: unable to read tree 3f19c0a" files={[]}
          busy={false} split={false} onSplit={() => {}} onRefresh={() => {}} onStage={() => {}} onDraft={none} />
      </Cell>
      <Cell cap="Staged and untracked">
        <TaskGitActions taskId="catalog" branch="brachio/rate-limit-7f2a" runner={runner} files={files}
          staged={["crates/brachio-auth/src/login.rs"]} untracked={["crates/brachio-auth/tests/limit.rs"]} conflicts={[]}
          onDraft={none} />
      </Cell>
      <Cell cap="Unresolved Git conflicts, commits blocked">
        <InspectorSection title={t("inspector.conflicts")}>
          <FileRow mark="C" path="src/store/cart.ts" />
          <FileRow mark="C" path="src/store/index.ts" />
          <InspectorNote>{t("inspector.conflictsStopped")}</InspectorNote>
        </InspectorSection>
        {git({}, [], ["src/store/cart.ts", "src/store/index.ts"])}
      </Cell>
      <Cell cap="Commit failed: a pre-commit hook">
        {git({ message: "feat(auth): limit login tries per IP",
          commit: { failure: { kind: "hook", output: "cargo fmt --check\nDiff in crates/brachio-auth/src/limiter.rs at line 41" } } })}
      </Cell>
      <Cell cap="Ask the agent for a commit message">{git({ asked: "commit" })}</Cell>
      <Cell cap="Commit done">{git({ commit: { ok: t("git.completed") } }, [])}</Cell>
      <Cell cap="Push rejected">{git({ push: { failure: { kind: "rejected", output: REJECTED } } }, [])}</Cell>
      <Cell cap="Push denied">
        {git({ push: { failure: { kind: "denied", repository: "4n5rud/brachio", output: DENIED } } }, [])}
      </Cell>
      <Cell cap="Pushed: write the pull request">
        {git({ push: { ok: t("git.pushed").replace("{branch}", "brachio/rate-limit-7f2a") }, prTitle: "Limit login tries per IP",
          prBody: "Adds a token bucket keyed by client IP.\n- 5 tries a minute per IP, then 429" }, [])}
      </Cell>
      <Cell cap="Create pull request failed: one already exists">
        {git({ prTitle: "Limit login tries per IP", prBody: "Adds a token bucket keyed by client IP.",
          pr: { failure: { kind: "exists", output: "", pullRequestNumber: 242, pullRequestUrl: "https://github.com/4n5rud/brachio/pull/242" } } }, [])}
      </Cell>
      <Cell cap="Merge check: idle"><MergeCheckView busy={false} onCheck={() => {}} /></Cell>
      <Cell cap="Merge check: checking"><MergeCheckView busy onCheck={() => {}} /></Cell>
      <Cell cap="Merge check: clean">
        <MergeCheckView busy={false} onCheck={() => {}} result={{ baseBranch: "origin/main", conflicts: [], remoteTracking: true }} />
      </Cell>
      <Cell cap="Merge check: conflicts">
        <MergeCheckView busy={false} onCheck={() => {}}
          result={{ baseBranch: "origin/main", conflicts: ["src/store/cart.ts", "src/store/index.ts"], remoteTracking: true }} />
      </Cell>
      <Cell cap="Merge check: local only">
        <MergeCheckView busy={false} onCheck={() => {}} result={{ baseBranch: "main", conflicts: [], remoteTracking: false }} />
      </Cell>
      <Cell cap="PR feedback: not loaded">{feedback({ loaded: false, items: [] })}</Cell>
      <Cell cap="PR feedback: loaded">{feedback()}</Cell>
      <Cell cap="PR feedback: nothing to fix">{feedback({ items: [] })}</Cell>
      <Cell cap="PR feedback: inline reviews failed">
        {feedback({ items: feedbackItems.slice(0, 1), inlineError: "GitHub answered 502." })}
      </Cell>
      <Cell cap="Pull request: none">{panel(report({ snapshot: null, noPullRequest: true }))}</Cell>
      <Cell cap="Pull request: checking GitHub">{panel()}</Cell>
      <Cell cap="Pull request: open, checks failed">{panel(report())}</Cell>
      <Cell cap="Pull request: draft, checks running">
        {panel(report({ snapshot: snapshot({ draft: true, review: "review_required", conflict: "unknown",
          checks: { overall: "pending", passing: 5, failing: 0, pending: 3, skipped: 0 }, failedChecks: [] }) }))}
      </Cell>
      <Cell cap="Pull request: merged">
        {panel(report({ snapshot: snapshot({ state: "merged", review: "approved", conflict: "clean",
          checks: { overall: "success", passing: 8, failing: 0, pending: 0, skipped: 0 }, failedChecks: [] }) }))}
      </Cell>
      <Cell cap="Pull request: closed, no checks">
        {panel(report({ snapshot: snapshot({ state: "closed", review: "not_required", conflict: "clean",
          checks: { overall: "none", passing: 0, failing: 0, pending: 0, skipped: 0 }, failedChecks: [] }) }))}
      </Cell>
      <Cell cap="Pull request: offline, last result">
        {panel(report({ error: "could not resolve host github.com", retryInSeconds: 20, checkedAtMs: NOW - 6 * 60_000 }))}
      </Cell>
      <Cell cap="Pull request: offline, no result yet">
        {panel(report({ snapshot: null, error: "could not resolve host github.com", checkedAtMs: null }))}
      </Cell>
      <Cell cap="Pull request: a different commit">
        {panel(report({ localHead: "9e1a2f0c3d4e5f60", headMatchesLocal: false }))}
      </Cell>
      <Cell cap="Pull request: could not check">{panel(undefined, "The GitHub CLI (gh) is not installed or not on PATH.")}</Cell>
      <Cell cap="Diff: unified, click a line number to comment" width={360}>
        <InspectorSection title={t("inspector.changedFiles")}>
          <DiffReview patch={PATCH} split={false} onSplit={() => {}} onDraft={none} />
        </InspectorSection>
      </Cell>
      <Cell cap="Diff: line comment" width={360}>
        <InspectorSection title={t("inspector.changedFiles")}>
          <DiffReview patch={PATCH} split={false} onSplit={() => {}} onDraft={none}
            initial={{ line: 14, comment: "Read the window from config." }} />
        </InspectorSection>
      </Cell>
      <Cell cap="Diff: the agent prompt is not ready" width={360}>
        <InspectorSection title={t("inspector.changedFiles")}>
          <DiffReview patch={PATCH} split={false} onSplit={() => {}} onDraft={none}
            initial={{ line: 14, comment: "Read the window from config.", error: t("diff.notReady") }} />
        </InspectorSection>
      </Cell>
      <Cell cap="Diff: side by side in the wide inspector" width={560}>
        <InspectorSection title={t("inspector.changedFiles")}>
          <DiffReview patch={PATCH} split onSplit={() => {}} onDraft={none} initial={{ line: 14, comment: "Read the window from config." }} />
        </InspectorSection>
      </Cell>
      <Cell cap="Draft check before paste">
        <InspectorSection title={t("diff.comment")}>
          <Button size="sm" disabled={opened()} onClick={() => void checkDraft()}>{t("draft.title")}</Button>
        </InspectorSection>
      </Cell>
    </div>
  );
}
