import type { CommandStatus } from "@sershi/contracts";
import { useEffect, useRef } from "react";

import { useI18n, type I18n } from "../../i18n";
import { applicationName, asApplicationResult, composeReply } from "../../i18n/domain";
import { useConversation, type Message, type Reply } from "../../state/conversation";
import { motionReduced } from "../../visual/appearance";
import { MicIcon } from "../shell/icons";
import styles from "./Transcript.module.css";

type Status = CommandStatus | "offline" | "voice";

const STATUS_WITH_LABEL = [
  "unavailable",
  "notUnderstood",
  "needsConfirmation",
  "denied",
  "failed",
  "offline",
  "rejected",
  "unresolved",
  "cancelled",
  "expired",
  "voice",
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
    case "voice":
      return "voice";
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
    case "voice":
      return reply.notice === "noSpeech" || reply.notice === "unclear"
        ? t(`voice.${reply.notice}`)
        : t(`voice.failure.${reply.notice}`);
  }
}

/** For an ambiguous application request: one button per candidate. */
function Candidates({ reply }: { reply: Reply }) {
  const { t } = useI18n();
  const submit = useConversation((s) => s.submit);
  if (reply.kind !== "outcome") return null;
  const result = asApplicationResult(reply.outcome.data);
  if (result?.kind !== "ambiguous") return null;
  const closing = reply.outcome.toolId === "system.close_application";
  return (
    <ul className={styles.candidates}>
      {result.candidates.map((app) => (
        <li key={app.id}>
          <button
            type="button"
            onClick={() => {
              // Re-ask with the exact Windows name, in the user's language.
              const command = closing ? "reply.apps.closeCommand" : "reply.apps.openCommand";
              void submit(t(command, { app: app.displayName }));
            }}
          >
            {applicationName(t, app)}
          </button>
        </li>
      ))}
    </ul>
  );
}

/** The session conversation. SERSHI's replies are announced to screen readers. */
export function Transcript({ messages }: { messages: Message[] }) {
  const end = useRef<HTMLDivElement>(null);
  const i18n = useI18n();
  const { t, format } = i18n;

  useEffect(() => {
    const reduce = motionReduced();
    end.current?.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "end" });
  }, [messages.length]);

  return (
    <div className={styles.scroller}>
      <ol className={styles.list} aria-live="polite" aria-relevant="additions">
        {messages.map((m) => {
          if (m.role === "user") {
            return (
              <li key={m.id} className={styles.message} data-role="user" data-via={m.via}>
                {m.via === "voice" && (
                  <p className={styles.via}>
                    <MicIcon />
                    {t("transcript.youSaid")}
                  </p>
                )}
                <p className={styles.user}>{m.text}</p>
              </li>
            );
          }
          const status = statusOf(m.reply);
          const outcome = m.reply.kind === "outcome" ? m.reply.outcome : null;
          return (
            <li key={m.id} className={styles.message} data-role="sershi" data-status={status}>
              <p className={styles.reply}>{replyText(i18n, m.reply)}</p>
              <Candidates reply={m.reply} />
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
