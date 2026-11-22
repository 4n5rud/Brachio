import "./CuesDialog.css";
import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { addCue, cuesOf, deleteCue, type CueKind } from "../lib/cues";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { IconButton } from "./ui/IconButton";
import { Input, TextArea } from "./ui/Input";
import { SegmentedControl } from "./ui/SegmentedControl";

/** A project's reusable prompts and shell commands: the list, and a form to add one. */
export function CuesDialog(props: { repoId: string; repoName: string; onClose: () => void }) {
  const [kind, setKind] = createSignal<CueKind>("prompt");
  const [label, setLabel] = createSignal("");
  const [body, setBody] = createSignal("");
  const [error, setError] = createSignal("");
  const add = async () => {
    setError("");
    try {
      await addCue(props.repoId, kind(), label(), body());
      setLabel("");
      setBody("");
    } catch (failure) {
      setError(String(failure));
    }
  };
  return (
    <Dialog title={<>{t("cue.title")}<span class="modal-sub">{props.repoName}</span></>} onClose={props.onClose} closeOnScrim escHint
      footer={<Button size="sm" onClick={() => props.onClose()}>{t("workspace.close")}</Button>}>
      <Show when={cuesOf(props.repoId).length > 0} fallback={<p class="form-hint">{t("cue.empty")}</p>}>
        <ul class="cue-list">
          <For each={cuesOf(props.repoId)}>{(cue) => (
            <li class="cue-row">
              <span class="cue-kind">{cue.kind === "prompt" ? t("cue.prompt") : t("cue.shell")}</span>
              <span class="cue-label" title={cue.body}>{cue.label}</span>
              <IconButton icon="x" size="sm" label={t("cue.delete")} onClick={() => void deleteCue(cue).catch((failure) => setError(String(failure)))} />
            </li>
          )}</For>
        </ul>
      </Show>
      <SegmentedControl label={t("cue.kind")} value={kind()} onChange={setKind}
        options={[{ value: "prompt", label: t("cue.prompt") }, { value: "shell", label: t("cue.shell") }]} />
      <Input label={t("cue.label")} value={label()} onInput={(event) => setLabel(event.currentTarget.value)} />
      <TextArea label={kind() === "prompt" ? t("cue.promptText") : t("cue.shellText")} mono={kind() === "shell"} rows={3}
        hint={kind() === "prompt" ? t("cue.promptHint") : t("cue.shellHint")}
        value={body()} onInput={(event) => setBody(event.currentTarget.value)} />
      <Show when={error()}><p class="form-error" role="alert">{error()}</p></Show>
      <Button size="sm" variant="primary" disabled={!label().trim() || !body().trim()} onClick={() => void add()}>{t("cue.add")}</Button>
    </Dialog>
  );
}
