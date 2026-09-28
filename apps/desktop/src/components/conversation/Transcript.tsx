import type { CommandStatus } from "@sershi/contracts";
import { useEffect, useRef } from "react";

import { useI18n, type I18n } from "../../i18n";
import { composeReply } from "../../i18n/domain";
import type { Message, Reply } from "../../state/conversation";
import styles from "./Transcript.module.css";

type Status = CommandStatus | "offline";

const STATUS_WITH_LABEL = [
  "unavailable",
  "notUnderstood",
  "needsConfirmation",
  "denied",
  "failed",
  "offline",
  "rejected",
] as const satisfies readonly Status[];

type LabelledStatus = (typeof STATUS_WITH_LABEL)[number];

const hasLabel = (s: Status): s is LabelledStatus =>
  (STATUS_WITH_LABEL as readonly Status[]).includes(s);

function statusOf(reply: Reply): Status {
  switch (reply.kind) {
    case "outcome":
      return reply.outcome.status;
    case "offline":
      return "offline";
    case "unreachable":
      return "failed";
  }
}

function replyText({ t, format }: I18n, reply: Reply): string {
  switch (reply.kind) {
    case "outcome":
      return composeReply(t, format, reply.outcome);
    case "offline":
      return t("reply.offline");
    case "unreachable":
      return t("reply.coreUnreachable");
  }
}

/** The session conversation. SERSHI's replies are announced to screen readers. */
export function Transcript({ messages }: { messages: Message[] }) {
  const end = useRef<HTMLDivElement>(null);
  const i18n = useI18n();
  const { t, format } = i18n;

  useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    end.current?.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "end" });
  }, [messages.length]);

  return (
    <div className={styles.scroller}>
      <ol className={styles.list} aria-live="polite" aria-relevant="additions">
        {messages.map((m) => {
          if (m.role === "user") {
            return (
              <li key={m.id} className={styles.message} data-role="user">
                <p className={styles.user}>{m.text}</p>
              </li>
            );
          }
          const status = statusOf(m.reply);
          const outcome = m.reply.kind === "outcome" ? m.reply.outcome : null;
          return (
            <li key={m.id} className={styles.message} data-role="sershi" data-status={status}>
              <p className={styles.reply}>{replyText(i18n, m.reply)}</p>
              <p className={styles.meta}>
                {hasLabel(status) && <span>{t(`transcript.status.${status}`)}</span>}
                {outcome?.toolId && <span className="t-mono">{outcome.toolId}</span>}
                {outcome?.durationMs != null && (
                  <span className="t-mono">{format.milliseconds(outcome.durationMs)}</span>
                )}
              </p>
            </li>
          );
        })}
      </ol>
      <div ref={end} />
    </div>
  );
}
