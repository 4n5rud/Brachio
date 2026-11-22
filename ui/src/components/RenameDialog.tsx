import { createSignal } from "solid-js";
import { t } from "../i18n";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Input } from "./ui/Input";

/** One text field to rename a task or a tab. Saving blank text is allowed only when `allowEmpty`. */
export function RenameDialog(props: {
  title: string;
  value: string;
  maxLength: number;
  hint?: string;
  allowEmpty?: boolean;
  onSave: (value: string) => void;
  onClose: () => void;
}) {
  const [value, setValue] = createSignal(props.value);
  let field!: HTMLInputElement;
  const canSave = () => props.allowEmpty || value().trim() !== "";
  const save = () => {
    if (canSave()) props.onSave(value().trim());
  };
  return (
    <Dialog title={props.title} size="confirm" centered onClose={props.onClose}
      initialFocus={() => field}
      footer={<>
        <Button variant="ghost" onClick={props.onClose}>{t("confirm.cancel")}</Button>
        <Button variant="primary" disabled={!canSave()} onClick={save}>{t("rename.save")}</Button>
      </>}>
      <form onSubmit={(event) => { event.preventDefault(); save(); }}>
        <Input ref={field} label={t("rename.label")} hint={props.hint} value={value()} maxLength={props.maxLength}
          onInput={(event) => setValue(event.currentTarget.value)} />
      </form>
    </Dialog>
  );
}
