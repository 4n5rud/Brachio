import { t } from "../i18n";
import { Button } from "./ui/Button";
import { EmptyState } from "./ui/EmptyState";

/** First-run screen. The only place the mascot appears at full size. */
export function EmptyRepo(props: { onAdd?: () => void }) {
  return (
    <EmptyState mascot title={t("empty.repo.title")} body={t("empty.repo.body")}>
      <Button variant="primary" icon="folder" onClick={() => props.onAdd?.()}>
        {t("empty.repo.add")}
      </Button>
    </EmptyState>
  );
}
