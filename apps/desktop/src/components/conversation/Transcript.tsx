import { useEffect, useRef } from "react";

import type { Message } from "../../state/conversation";
import styles from "./Transcript.module.css";

const STATUS_LABEL: Partial<Record<NonNullable<Message["status"]>, string>> = {
  unavailable: "Not available yet",
  notUnderstood: "Not understood",
  needsConfirmation: "Needs approval",
  denied: "Blocked by policy",
  failed: "Failed",
  offline: "Core not connected",
  rejected: "Not sent",
};

/** The session conversation. SERSHI's replies are announced to screen readers. */
export function Transcript({ messages }: { messages: Message[] }) {
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    end.current?.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "end" });
  }, [messages.length]);

  return (
    <div className={styles.scroller}>
      <ol className={styles.list} aria-live="polite" aria-relevant="additions">
        {messages.map((m) => (
          <li key={m.id} className={styles.message} data-role={m.role} data-status={m.status}>
            {m.role === "user" ? (
              <p className={styles.user}>{m.text}</p>
            ) : (
              <>
                <p className={styles.reply}>{m.text}</p>
                <p className={styles.meta}>
                  {m.status && STATUS_LABEL[m.status] && <span>{STATUS_LABEL[m.status]}</span>}
                  {m.toolId && <span className="t-mono">{m.toolId}</span>}
                  {m.durationMs != null && <span className="t-mono">{m.durationMs} ms</span>}
                </p>
              </>
            )}
          </li>
        ))}
      </ol>
      <div ref={end} />
    </div>
  );
}
