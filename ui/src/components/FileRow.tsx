import { Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { DiffStat } from "./ui/DiffStat";
import "./FileRow.css";

export type FileMark = "A" | "M" | "D" | "R" | "U" | "C" | "failed";

const MARK_CLASS: Record<FileMark, string> = { A: "a", M: "m", D: "d", R: "r", U: "u", C: "c", failed: "c" };

/** One file: how it changed, its name with the folder after it, its line totals and one verb. */
export function FileRow(props: {
  mark?: FileMark;
  path: string;
  /** Replaces the file name and folder, for rows that are not a path. */
  label?: string;
  additions?: number;
  deletions?: number;
  actionLabel?: string;
  onAction?: () => void;
  actionDisabled?: boolean;
  /** Makes the whole row a button, for lists a person picks from. */
  onSelect?: () => void;
  selected?: boolean;
}) {
  const split = () => {
    const at = props.path.lastIndexOf("/");
    return at < 0 ? { name: props.path, dir: "" } : { name: props.path.slice(at + 1), dir: props.path.slice(0, at) };
  };
  return (
    <Dynamic component={props.onSelect ? "button" : "div"} class="file-row" data-current={props.selected || undefined}
      title={props.path} type={props.onSelect ? "button" : undefined} aria-pressed={props.onSelect ? !!props.selected : undefined}
      onClick={() => props.onSelect?.()}>
      <Show when={props.mark}>
        {(mark) => <span class={`fstat fstat-${MARK_CLASS[mark()]}`}>{mark() === "failed" ? "✕" : mark()}</span>}
      </Show>
      <span class="name">
        {props.label ?? split().name}
        <Show when={!props.label && split().dir}><span class="dir">{split().dir}</span></Show>
      </span>
      <Show when={props.additions !== undefined && props.deletions !== undefined}>
        <DiffStat additions={props.additions} deletions={props.deletions} />
      </Show>
      <Show when={props.actionLabel}>
        {(label) => (
          <button type="button" class="act" disabled={props.actionDisabled} onClick={() => props.onAction?.()}>
            {label()}
          </button>
        )}
      </Show>
    </Dynamic>
  );
}
