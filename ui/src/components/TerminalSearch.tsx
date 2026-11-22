import { createSignal } from "solid-js";
import { t } from "../i18n";
import { IconButton } from "./ui/IconButton";
import { Input } from "./ui/Input";
import "./TerminalSearch.css";

/** Find bar over a terminal: Enter finds the next match, Shift+Enter the previous, Esc closes. */
export function TerminalSearch(props: {
  /** Returns whether anything matched. */
  find: (query: string, backwards: boolean) => boolean;
  onClose: () => void;
}) {
  const [query, setQuery] = createSignal("");
  const [missed, setMissed] = createSignal(false);
  const run = (backwards: boolean) => setMissed(query() !== "" && !props.find(query(), backwards));
  return (
    <div class="terminal-search" role="search">
      <Input compact mono aria-label={t("terminal.search")} placeholder={t("terminal.search")} value={query()}
        invalid={missed()} ref={(element: HTMLInputElement) => queueMicrotask(() => element.focus())}
        onInput={(event) => { setQuery(event.currentTarget.value); run(false); }}
        onKeyDown={(event) => {
          if (event.isComposing) return;
          if (event.key === "Enter") { event.preventDefault(); run(event.shiftKey); }
          else if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); props.onClose(); }
        }} />
      <span class="terminal-search-miss" aria-live="polite">{missed() ? t("terminal.searchNone") : ""}</span>
      <IconButton size="sm" icon="chevronDown" label={t("terminal.searchNext")} onClick={() => run(false)} />
      <IconButton size="sm" icon="x" label={t("terminal.searchClose")} onClick={props.onClose} />
    </div>
  );
}
