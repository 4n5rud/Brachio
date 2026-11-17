import { t } from "../i18n";
import { NECK } from "./BrandMark";
import { Icon } from "./Icon";

/** First-run screen. The only place the mascot appears at full size. */
export function EmptyRepo() {
  return (
    <section class="empty-state">
      <div class="peek">
        <svg viewBox="300 360 640 830" aria-hidden="true">
          <path d={NECK} fill="#98E0BF" />
          <ellipse cx="680" cy="525" rx="25" ry="40" fill="#1F1F24" />
          <ellipse cx="836" cy="525" rx="25" ry="40" fill="#1F1F24" />
        </svg>
      </div>
      <h1>{t("empty.repo.title")}</h1>
      <p>{t("empty.repo.body")}</p>
      <button class="button button-primary" type="button">
        <Icon name="folder" />
        {t("empty.repo.add")}
      </button>
    </section>
  );
}
