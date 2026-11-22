import { For, Show, createSignal } from "solid-js";
import { t } from "../../i18n";
import { alertSoundSettings, previewAlertSound, readCustomSound, updateAlertSound, type AlertSoundKind } from "../../lib/alert-sound";
import { Button } from "../ui/Button";
import { Toggle } from "../ui/Toggle";
import { SettingRow } from "./SettingRow";
import "./AlertSoundSettings.css";

const KINDS: readonly AlertSoundKind[] = ["finished", "ask"];

function AlertSoundRows(props: { kind: AlertSoundKind }) {
  const [problem, setProblem] = createSignal("");
  let picker!: HTMLInputElement;
  const setting = () => alertSoundSettings()[props.kind];
  const choose = async (file: File | undefined) => {
    if (!file) return;
    try {
      const result = await readCustomSound(file);
      if (result === "type") setProblem(t("sound.fileType"));
      else if (result === "size") setProblem(t("sound.fileSize"));
      else setProblem(updateAlertSound(props.kind, { custom: result }) ? "" : t("sound.fileUnsaved"));
    } catch {
      setProblem(t("sound.fileUnreadable"));
    }
    picker.value = "";
  };
  return (
    <>
      <Toggle checked={setting().on} label={t(props.kind === "ask" ? "sound.ask" : "sound.finished")}
        hint={t(props.kind === "ask" ? "sound.askHint" : "sound.finishedHint")}
        onChange={(on) => updateAlertSound(props.kind, { on })} />
      <SettingRow label={t("sound.volume")}>
        <input class="sound-volume" type="range" min="0" max="100" step="5" disabled={!setting().on}
          aria-label={t("sound.volume")} value={Math.round(setting().volume * 100)}
          onInput={(event) => updateAlertSound(props.kind, { volume: event.currentTarget.valueAsNumber / 100 })} />
        <Button size="sm" disabled={!setting().on} onClick={() => void previewAlertSound(props.kind)}>{t("sound.test")}</Button>
      </SettingRow>
      <SettingRow label={setting().custom?.name ?? t("sound.builtIn")} hint={problem() || undefined} error={!!problem()}>
        <input ref={picker} class="sound-file" type="file" accept=".wav,.mp3,.ogg,audio/wav,audio/mpeg,audio/ogg" tabIndex={-1}
          onChange={(event) => void choose(event.currentTarget.files?.[0])} />
        <Show when={setting().custom}>
          <Button size="sm" variant="ghost" onClick={() => { updateAlertSound(props.kind, { custom: null }); setProblem(""); }}>
            {t("sound.reset")}
          </Button>
        </Show>
        <Button size="sm" onClick={() => picker.click()}>{t("sound.chooseFile")}</Button>
      </SettingRow>
    </>
  );
}

/** Per-sound switch, volume, test and custom file for the finished and question sounds. */
export function AlertSoundSettings() {
  return (
    <>
      <For each={KINDS}>{(kind) => <AlertSoundRows kind={kind} />}</For>
      <p class="form-hint">{t("sound.note")}</p>
    </>
  );
}
