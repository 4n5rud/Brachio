import { createResource } from "solid-js";
import { t } from "../i18n";
import { branchWhere, filterBranches, listBranches } from "../lib/branch-filter";
import { Combobox } from "./ui/Combobox";

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
  const options = () =>
    filterBranches(branches() ?? [], props.value).map((branch) => ({
      value: branch.value,
      label: branch.name,
      detail: branchWhere(branch, t("newTask.branchLocal"), t("newTask.branchAnd")),
    }));
  return (
    <Combobox mono label={t("newTask.base")} listLabel={t("newTask.branches")} value={props.value}
      placeholder={props.placeholder} error={props.error} options={options()} onValue={props.onValue} />
  );
}
