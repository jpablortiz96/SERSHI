import { KIND_TONE, TONE_GLYPH } from "../../components/activity/ActivityItem";
import { Glyph } from "../../components/core/StateGlyph";
import { useI18n } from "../../i18n";
import { describeActivity, toolName } from "../../i18n/domain";
import { useActivity } from "../../state/activity";
import styles from "./Page.module.css";

export function ActivityView() {
  const entries = useActivity((s) => s.entries);
  const { t, format } = useI18n();

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>{t("activity.title")}</h1>
        <p className={styles.lede}>{t("activity.lede")}</p>
      </header>

      {entries.length === 0 ? (
        <p className={styles.empty}>{t("activity.empty")}</p>
      ) : (
        <table className={styles.table}>
          <thead>
            <tr>
              <th scope="col">{t("activity.columns.time")}</th>
              <th scope="col">{t("activity.columns.status")}</th>
              <th scope="col">{t("activity.columns.event")}</th>
              <th scope="col">{t("activity.columns.tool")}</th>
              <th scope="col" className={styles.num}>
                {t("activity.columns.duration")}
              </th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <tr key={e.id} data-tone={KIND_TONE[e.kind]}>
                <td className="t-mono">{format.time(e.atMs)}</td>
                <td className={styles.status}>
                  <Glyph shape={TONE_GLYPH[KIND_TONE[e.kind]]} />
                  {t(`activity.tones.${KIND_TONE[e.kind]}`)}
                </td>
                <td>{describeActivity(t, e)}</td>
                <td className="t-mono" title={e.toolId ? toolName(t, e.toolId) : undefined}>
                  {e.toolId ?? "—"}
                </td>
                <td className={`t-mono ${styles.num}`}>
                  {e.durationMs != null ? format.milliseconds(e.durationMs) : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className={styles.footnote}>{t("activity.footnote")}</p>
    </div>
  );
}
