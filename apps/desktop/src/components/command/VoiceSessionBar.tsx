import { useI18n } from "../../i18n";
import { useVoice } from "../../state/voice";
import styles from "./VoiceSessionBar.module.css";

/**
 * The voice session indicator (Gate 4.1). Visible whenever a session is
 * active, so hands-free listening is never hidden: what SERSHI is doing,
 * how to finish, and an End button. Colors follow SERSHI's own state
 * tokens — no separate visual system.
 */
export function VoiceSessionBar() {
  const { t } = useI18n();
  const session = useVoice((s) => s.session);
  const stopSession = useVoice((s) => s.stopSession);
  if (!session?.phase) return null;
  const phase = session.phase;
  return (
    <div className={styles.bar} data-phase={phase} role="status" aria-live="polite">
      <span className={styles.dot} aria-hidden="true" />
      <span className={styles.label}>{t("session.label")}</span>
      <span className={styles.phase}>{t(`session.phase.${phase}`)}</span>
      <span className={styles.hint}>{t("session.hint")}</span>
      <button
        type="button"
        className={styles.end}
        onClick={stopSession}
        aria-label={t("command.sessionStop")}
      >
        {t("session.end")}
      </button>
    </div>
  );
}
