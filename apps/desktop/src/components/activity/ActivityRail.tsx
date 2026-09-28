import { useI18n } from "../../i18n";
import { useActivity } from "../../state/activity";
import rail from "../shell/Rail.module.css";
import styles from "./Activity.module.css";
import { ActivityItem } from "./ActivityItem";

const VISIBLE = 9;

export function ActivityRail({ onViewAll }: { onViewAll: () => void }) {
  const entries = useActivity((s) => s.entries);
  const visible = entries.slice(0, VISIBLE);
  const { t } = useI18n();

  return (
    <aside className={rail.rail} aria-labelledby="activity-heading">
      <div className={styles.railHead}>
        <h2 id="activity-heading" className="t-label">
          {t("activity.recent")}
        </h2>
        {entries.length > 0 && (
          <button type="button" className={styles.link} onClick={onViewAll}>
            {t("activity.viewAll")}
          </button>
        )}
      </div>
      {visible.length === 0 ? (
        <p className={rail.empty}>{t("activity.railEmpty")}</p>
      ) : (
        <ol className={styles.timeline}>
          {visible.map((e) => (
            <ActivityItem key={e.id} entry={e} />
          ))}
        </ol>
      )}
    </aside>
  );
}
