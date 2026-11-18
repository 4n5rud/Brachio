export function DiffStat(props: { additions: number; deletions: number }) {
  return (
    <span class="diffstat">
      <span class="diffstat-add">+{props.additions}</span>
      <span class="diffstat-del">−{props.deletions}</span>
    </span>
  );
}
