import { For, Show, createResource, createSignal, createUniqueId } from "solid-js";
import { Portal } from "solid-js/web";
import { t } from "../i18n";
import { branchWhere, filterBranches, listBranches } from "../lib/branch-filter";
import { Input } from "./ui/Input";
import "./ui/Menu.css";

/** The base branch field of the new task dialog: typing filters the project's branches, and
 * any other text is still accepted so a commit or tag works. */
export function BranchField(props: {
  repoId: string;
  value: string;
  placeholder: string;
  error?: string;
  onValue: (value: string) => void;
}) {
  const [branches] = createResource(
    () => props.repoId,
    (id) => listBranches(id).catch(() => []),
  );
  const listId = createUniqueId();
  const [open, setOpen] = createSignal(false);
  const [active, setActive] = createSignal(0);
  const [box, setBox] = createSignal({ left: 0, top: 0, width: 0 });
  const shown = () => (open() ? filterBranches(branches() ?? [], props.value) : []);
  const optionId = (index: number) => `${listId}-${index}`;

  const show = (input: HTMLInputElement) => {
    const rect = input.getBoundingClientRect();
    setBox({ left: rect.left, top: rect.bottom + 4, width: rect.width });
    setOpen(true);
  };
  const pick = (value: string) => {
    props.onValue(value);
    setOpen(false);
  };
  const onKeyDown = (event: KeyboardEvent & { currentTarget: HTMLInputElement }) => {
    if (event.isComposing) return;
    if (event.key === "Escape" && open()) {
      event.preventDefault();
      setOpen(false);
      return;
    }
    const count = shown().length;
    if ((event.key === "ArrowDown" || event.key === "ArrowUp") && count > 0) {
      event.preventDefault();
      if (!open()) show(event.currentTarget);
      setActive((index) => (index + (event.key === "ArrowDown" ? 1 : -1) + count) % count);
    } else if (event.key === "Enter" && open() && count > 0 && active() < count) {
      const chosen = shown()[active()];
      if (chosen && chosen.value !== props.value) {
        event.preventDefault();
        pick(chosen.value);
      } else {
        setOpen(false);
      }
    }
  };

  return (
    <>
      <Input mono label={t("newTask.base")} value={props.value} placeholder={props.placeholder}
        error={props.error} role="combobox" autocomplete="off" aria-expanded={shown().length > 0}
        aria-controls={listId} aria-autocomplete="list"
        aria-activedescendant={shown().length > 0 ? optionId(active()) : undefined}
        onFocus={(event) => show(event.currentTarget)}
        onBlur={() => setOpen(false)}
        onKeyDown={onKeyDown}
        onInput={(event) => {
          props.onValue(event.currentTarget.value);
          setActive(0);
          show(event.currentTarget);
        }} />
      <Show when={shown().length > 0}>
        <Portal>
          <div id={listId} class="menu" role="listbox" aria-label={t("newTask.branches")}
            style={{ left: `${box().left}px`, top: `${box().top}px`, "min-width": `${box().width}px` }}>
            <div class="menu-label">{t("newTask.branches")}</div>
            <For each={shown()}>
              {(branch, index) => (
                <div id={optionId(index())} class="menu-item" classList={{ on: index() === active() }}
                  role="option" aria-selected={index() === active()}
                  onMouseDown={(event) => { event.preventDefault(); pick(branch.value); }}>
                  {branch.name}
                  <small>{branchWhere(branch, t("newTask.branchLocal"), t("newTask.branchAnd"))}</small>
                </div>
              )}
            </For>
          </div>
        </Portal>
      </Show>
    </>
  );
}
