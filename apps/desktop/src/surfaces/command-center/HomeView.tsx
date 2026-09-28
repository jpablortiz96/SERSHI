import { ActivityRail } from "../../components/activity/ActivityRail";
import { CommandBar } from "../../components/command/CommandBar";
import { Transcript } from "../../components/conversation/Transcript";
import { Core } from "../../components/core/Core";
import { SystemRail } from "../../components/telemetry/SystemRail";
import { useI18n } from "../../i18n";
import { stateLabel, stateLine } from "../../i18n/domain";
import { useAssistantStore, useDisplayState } from "../../state/assistant";
import { useConversation } from "../../state/conversation";
import styles from "./HomeView.module.css";

const SUGGESTIONS = [
  "home.suggestions.memory",
  "home.suggestions.cpu",
  "home.suggestions.system",
] as const;

export function HomeView({ onViewActivity }: { onViewActivity: () => void }) {
  const state = useDisplayState();
  const revision = useAssistantStore((s) => s.snapshot.revision);
  const { messages, submit } = useConversation();
  const conversing = messages.length > 0;
  const { t } = useI18n();

  return (
    <div className={styles.home}>
      <SystemRail />

      <section className={styles.stage} data-conversing={conversing || undefined}>
        <div className={styles.presence}>
          <div className={styles.coreSlot}>
            <Core state={state} size={232} pulseKey={revision} className={styles.core} />
          </div>
          <p className={styles.status} role="status">
            <span className={styles.statusLabel}>{stateLabel(t, state)}</span>
            <span className={styles.statusLine}>{stateLine(t, state)}</span>
          </p>
        </div>

        <div className={styles.dialogue}>
          {conversing ? (
            <Transcript messages={messages} />
          ) : (
            <div className={styles.welcome}>
              <h1 className={styles.greeting}>{t("home.greeting")}</h1>
              <ul className={styles.suggestions} aria-label={t("home.suggestionsLabel")}>
                {SUGGESTIONS.map((key) => (
                  <li key={key}>
                    <button
                      type="button"
                      onClick={() => {
                        void submit(t(key));
                      }}
                    >
                      {t(key)}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>

        <CommandBar />
      </section>

      <ActivityRail onViewAll={onViewActivity} />
    </div>
  );
}
