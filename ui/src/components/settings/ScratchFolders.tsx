import { For, Show, createResource, createSignal } from "solid-js";
import { open as openFolder } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";
import { convertScratch, discardScratch, listScratchFolders, PROJECTS_CHANGED, type ScratchFolder } from "../../lib/scratch";
import { Button } from "../ui/Button";
import { confirmDialog } from "../ui/ConfirmDialog";
import { Dialog } from "../ui/Dialog";
import { Input } from "../ui/Input";
import { SettingRow } from "./SettingRow";
import { SettingsGroup } from "./SettingsGroup";

const PARENT_KEY = "brachio.projectParent";

function savedParent(): string {
  try {
    return localStorage.getItem(PARENT_KEY) ?? "";
  } catch {
    return "";
  }
}

function ConvertDialog(props: { folder: ScratchFolder; onDone: () => void; onClose: () => void }) {
  const [parent, setParent] = createSignal(savedParent());
  const [name, setName] = createSignal(props.folder.name.replace(/-[0-9a-f]{6}$/, ""));
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const ready = () => !busy() && parent().trim() !== "" && name().trim() !== "";
  const choose = async () => {
    try {
      const picked = await openFolder({ directory: true, multiple: false, defaultPath: parent() || undefined });
      if (typeof picked === "string") setParent(picked);
    } catch (failure) {
      setError(String(failure));
    }
  };
  const submit = async () => {
    const repoId = props.folder.repoId;
    if (!ready() || !repoId) return;
    setBusy(true);
    setError(undefined);
    try {
      await convertScratch(repoId, parent().trim(), name().trim());
      try {
        localStorage.setItem(PARENT_KEY, parent().trim());
      } catch {
        // The location is only a convenience for next time.
      }
      props.onDone();
    } catch (failure) {
      setError(String(failure));
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("scratch.convertTitle")} onClose={() => { if (!busy()) props.onClose(); }} escHint
      footer={
        <>
          <Button variant="ghost" disabled={busy()} onClick={props.onClose}>{t("newTask.cancel")}</Button>
          <Button variant="primary" disabled={!ready()} loading={busy()} loadingLabel={t("scratch.converting")}
            onClick={() => void submit()}>{t("scratch.convertDo")}</Button>
        </>
      }>
      <div class="form-field">
        <p class="form-hint">{t("scratch.convertNote")}</p>
        <Input label={t("project.projectName")} value={name()} disabled={busy()} onInput={(event) => setName(event.currentTarget.value)} />
        <div class="location-row">
          <Input mono label={t("project.location")} value={parent()} disabled={busy()} onInput={(event) => setParent(event.currentTarget.value)} />
          <Button disabled={busy()} onClick={() => void choose()}>{t("project.locationChoose")}</Button>
        </div>
      </div>
      <Show when={error()}>
        {(message) => (
          <div class="create-failed" role="alert">
            <pre class="create-log">{message()}</pre>
          </div>
        )}
      </Show>
    </Dialog>
  );
}

/** Scratch folders that were not turned into projects, with the two ways to finish with one. */
export function ScratchFolders() {
  const [folders, { refetch }] = createResource(listScratchFolders);
  const [converting, setConverting] = createSignal<ScratchFolder>();
  const [error, setError] = createSignal<string>();
  const changed = () => {
    setConverting(undefined);
    void refetch();
    window.dispatchEvent(new Event(PROJECTS_CHANGED));
  };
  const discard = async (folder: ScratchFolder) => {
    const confirmed = await confirmDialog({
      title: t("scratch.discardTitle"),
      body: `${t("scratch.discardBody")}\n\n${folder.path}`,
      confirmLabel: t("scratch.discardDo"),
    });
    if (!confirmed) return;
    try {
      await discardScratch(folder.path);
      setError(undefined);
      changed();
    } catch (failure) {
      setError(String(failure));
    }
  };
  const hint = (folder: ScratchFolder) =>
    [folder.missing ? t("scratch.missing") : null, folder.repoId === null ? t("scratch.leftOver") : null, folder.tasks.join(", ") || null, folder.path]
      .filter(Boolean).join(" · ");
  return (
    <>
      <p class="form-hint">{t("scratch.intro")}</p>
      <Show when={error()}>{(message) => <p class="form-hint" role="alert">{message()}</p>}</Show>
      <Show when={(folders() ?? []).length > 0} fallback={<p class="form-hint">{t("scratch.none")}</p>}>
        <SettingsGroup>
          <For each={folders()}>{(folder) => (
            <SettingRow label={folder.name} hint={hint(folder)}>
              <div class="row-gap">
                <Show when={folder.repoId !== null && !folder.missing}>
                  <Button size="sm" onClick={() => setConverting(folder)}>{t("scratch.convert")}</Button>
                </Show>
                <Button size="sm" variant="ghost" onClick={() => void discard(folder)}>{t("scratch.discard")}</Button>
              </div>
            </SettingRow>
          )}</For>
        </SettingsGroup>
      </Show>
      <Show when={converting()}>
        {(folder) => <ConvertDialog folder={folder()} onDone={changed} onClose={() => setConverting(undefined)} />}
      </Show>
    </>
  );
}
