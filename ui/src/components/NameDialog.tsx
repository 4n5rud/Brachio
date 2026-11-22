import { createSignal } from "solid-js";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Input } from "./ui/Input";
import { t } from "../i18n";

/** Asks for one file or folder name; Enter confirms. */
export function NameDialog(props: {
  title: string;
  label: string;
  initial?: string;
  confirmLabel: string;
  onSubmit: (name: string) => void;
  onClose: () => void;
}) {
  const [name, setName] = createSignal(props.initial ?? "");
  let field: HTMLInputElement | undefined;
  const submit = () => {
    const value = name().trim();
    if (value) props.onSubmit(value);
  };
  return (
    <Dialog title={props.title} size="confirm" onClose={props.onClose} initialFocus={() => field}
      footer={
        <>
          <Button variant="ghost" onClick={props.onClose}>{t("confirm.cancel")}</Button>
          <Button variant="primary" disabled={name().trim() === ""} onClick={submit}>{props.confirmLabel}</Button>
        </>
      }>
      <Input ref={(element: HTMLInputElement) => {
        field = element;
        queueMicrotask(() => { element.select(); });
      }} aria-label={props.label} placeholder={props.label} mono value={name()}
        onInput={(event) => setName(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.isComposing) { event.preventDefault(); submit(); }
        }} />
    </Dialog>
  );
}
