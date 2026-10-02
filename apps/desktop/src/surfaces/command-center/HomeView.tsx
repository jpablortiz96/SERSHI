import { useLayoutEffect, useRef } from "react";

import { ActivityRail } from "../../components/activity/ActivityRail";
import { CommandBar } from "../../components/command/CommandBar";
import { Transcript } from "../../components/conversation/Transcript";
import { Core } from "../../components/core/Core";
import { StateGlyph } from "../../components/core/StateGlyph";
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
  "home.suggestions.notepad",
] as const;

export function HomeView({ onViewActivity }: { onViewActivity: () => void }) {
  const state = useDisplayState();
  const revision = useAssistantStore((s) => s.snapshot.revision);
  const { messages, submit, newConversation, livePlan } = useConversation();
  const conversing = messages.length > 0;
  const { t } = useI18n();
  const coreSlot = useRef<HTMLDivElement>(null);

  // The core is the room's light source: tell the ambient field where it is
  // (it moves with window size and when the conversation starts).
  useLayoutEffect(() => {
    const slot = coreSlot.current;
    if (!slot) return;
    const place = () => {
      const rect = slot.getBoundingClientRect();
      document.documentElement.style.setProperty(
        "--core-y",
        `${Math.round(rect.top + rect.height / 2)}px`,
      );
    };
    place();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(place);
    observer?.observe(slot);
    window.addEventListener("resize", place);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", place);
    };
  }, []);

  return (
    <div className={styles.home}>
      <SystemRail />

      <section className={styles.stage} data-conversing={conversing || undefined}>
        <div className={styles.presence}>
          <div ref={coreSlot} className={styles.coreSlot}>
            <Core state={state} variant="hero" pulseKey={revision} className={styles.core} />
          </div>
          <p className={styles.status} role="status">
            <span className={styles.statusLabel}>
              <StateGlyph state={state} />
              {stateLabel(t, state)}
            </span>
            <span className={styles.statusLine}>{stateLine(t, state)}</span>
          </p>
        </div>

        <div className={styles.dialogue}>
          {conversing && (
            <div className={styles.conversationBar}>
              <button type="button" className={styles.newConversation} onClick={newConversation}>
                {t("home.newConversation")}
              </button>
            </div>
          )}
          {conversing ? (
            <Transcript messages={messages} livePlan={livePlan} />
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
