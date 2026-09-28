import { ActivityRail } from "../../components/activity/ActivityRail";
import { CommandBar } from "../../components/command/CommandBar";
import { Transcript } from "../../components/conversation/Transcript";
import { Core } from "../../components/core/Core";
import { STATE_COPY } from "../../components/core/stateCopy";
import { SystemRail } from "../../components/telemetry/SystemRail";
import { useAssistantStore, useDisplayState } from "../../state/assistant";
import { useConversation } from "../../state/conversation";
import styles from "./HomeView.module.css";

const SUGGESTIONS = [
  "How much memory am I using?",
  "What's my processor load?",
  "Tell me about this computer",
];

export function HomeView({ onViewActivity }: { onViewActivity: () => void }) {
  const state = useDisplayState();
  const revision = useAssistantStore((s) => s.snapshot.revision);
  const { messages, submit } = useConversation();
  const conversing = messages.length > 0;

  return (
    <div className={styles.home}>
      <SystemRail />

      <section className={styles.stage} data-conversing={conversing || undefined}>
        <div className={styles.presence}>
          <div className={styles.coreSlot}>
            <Core state={state} size={232} pulseKey={revision} className={styles.core} />
          </div>
          <p className={styles.status} role="status">
            <span className={styles.statusLabel}>{STATE_COPY[state].label}</span>
            <span className={styles.statusLine}>{STATE_COPY[state].line}</span>
          </p>
        </div>

        <div className={styles.dialogue}>
          {conversing ? (
            <Transcript messages={messages} />
          ) : (
            <div className={styles.welcome}>
              <h1 className={styles.greeting}>How can I help?</h1>
              <ul className={styles.suggestions} aria-label="Suggestions">
                {SUGGESTIONS.map((s) => (
                  <li key={s}>
                    <button
                      type="button"
                      onClick={() => {
                        void submit(s);
                      }}
                    >
                      {s}
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
