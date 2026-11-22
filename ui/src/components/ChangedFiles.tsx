import { Button } from "./ui/Button";
import { DiffStat } from "./ui/DiffStat";
import { IconButton } from "./ui/IconButton";
import { For, Show } from "solid-js";
import { t } from "../i18n";
import type { DiffFile } from "../lib/diff";
import { gitOutput } from "../lib/git-failure";
import { DiffReview } from "./DiffReview";
import { FileRow } from "./FileRow";
import { InspectorNote, OutputLog } from "./InspectorNote";
import { ActionRow, InspectorSection } from "./InspectorSection";

export interface ChangesData {
  patch: string;
  staged: readonly string[];
  untracked: readonly string[];
  conflicts: readonly string[];
  operation?: "rebase" | "merge" | null;
  truncated: boolean;
}

/** What the branch changed: loading, unreadable, empty, or its files with the diff under them. */
export function ChangedFiles(props: {
  additions?: number;
  deletions?: number;
  loading: boolean;
  error?: string;
  data?: ChangesData;
  files: readonly DiffFile[];
  busy: boolean;
  split: boolean;
  onSplit: (split: boolean) => void;
  onRefresh: () => void;
  onStage: (path: string) => void;
  onDraft: (text: string) => Promise<boolean>;
}) {
  const staged = (path: string) => props.data?.staged.includes(path) ?? false;
  return (
    <InspectorSection title={t("inspector.changedFiles")}
      end={<>
        <DiffStat additions={props.additions} deletions={props.deletions} />
        <IconButton size="sm" icon="refresh" label={t("inspector.refresh")} busy={props.loading} onClick={props.onRefresh} />
      </>}>
      <Show when={props.loading && !props.data}>
        <InspectorNote tone="progress">{t("inspector.loadingChanges")}</InspectorNote>
      </Show>
      <Show when={props.error}>
        {(error) => (
          <>
            <InspectorNote tone="error">{t("inspector.changesFailed")}</InspectorNote>
            <OutputLog text={gitOutput(error())} />
            <ActionRow>
              <Button size="sm" icon="refresh" busy={props.loading} onClick={props.onRefresh}>{t("inspector.tryAgain")}</Button>
            </ActionRow>
          </>
        )}
      </Show>
      <Show when={!props.data && !props.error && !props.loading && props.additions === undefined}>
        <InspectorNote tone="warn">{t("diff.totalsUnavailable")}</InspectorNote>
      </Show>
      <Show when={props.data}>
        {(data) => (
          <>
            <For each={props.files}>{(file) => (
              <FileRow mark={file.status} path={file.path} additions={file.additions} deletions={file.deletions}
                actionLabel={staged(file.path) ? undefined : t("git.stageFile")} actionDisabled={props.busy}
                onAction={() => props.onStage(file.path)} />
            )}</For>
            <Show when={data().patch}>
              <DiffReview patch={data().patch} split={props.split} onSplit={props.onSplit} onDraft={props.onDraft} />
            </Show>
            <Show when={data().truncated}><InspectorNote>{t("inspector.truncated")}</InspectorNote></Show>
            <Show when={!data().patch && data().untracked.length === 0 && data().staged.length === 0 && data().conflicts.length === 0}>
              <InspectorNote>{t("inspector.noChanges")}</InspectorNote>
            </Show>
            <Show when={data().patch}><InspectorNote>{t("inspector.diffBase")}</InspectorNote></Show>
          </>
        )}
      </Show>
    </InspectorSection>
  );
}
