import type { CommandStatus } from "@sershi/contracts";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

import { useI18n, type I18n } from "../../i18n";
import type { PlanReport } from "@sershi/contracts";

import {
  applicationName,
  asApplicationResult,
  composeReply,
  stepLabel,
  understoodAction,
} from "../../i18n/domain";
import { useConversation, type Message, type Reply } from "../../state/conversation";
import { motionReduced } from "../../visual/appearance";
import { ArrowDownIcon, MicIcon } from "../shell/icons";
import styles from "./Transcript.module.css";

type Status = CommandStatus | "offline" | "voice";

const STATUS_WITH_LABEL = [
  "partial",
  "needsClarification",
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
    case "session":
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
    case "session":
      return reply.notice === "listening" || reply.notice === "anythingElse"
        ? t(`session.${reply.notice}`)
        : t(`session.ended.${reply.notice}`);
  }
}

/** How close to the end counts as "reading the latest" (px). */
const NEAR_END_PX = 48;

/**
 * Answers to a question SERSHI asked: one button per offered application
 * (sent as its exact name, which only selects among the offered ones), or
 * Yes/No for "Did you mean …?". A button only answers the question; closing
 * still needs the trusted confirmation window.
 */
function Candidates({ reply, latest }: { reply: Reply; latest: boolean }) {
  const { t } = useI18n();
  const submit = useConversation((s) => s.submit);
  if (reply.kind !== "outcome") return null;
  const detail = reply.outcome.detail;
  // The Agent Brain's question: its options are trusted applications.
  if (detail?.kind === "brainQuestion" && detail.options.length > 0) {
    if (!latest) return null;
    return (
      <ul className={styles.candidates}>
        {detail.options.map((app) => (
          <li key={app.id}>
            <button type="button" onClick={() => void submit(app.displayName)}>
              {applicationName(t, app)}
            </button>
          </li>
        ))}
      </ul>
    );
  }
  if (detail?.kind === "clarification") {
    // Only the latest question can still be answered.
    if (!latest) return null;
    const c = detail.clarification;
    if (c.kind === "didYouMean") {
      return (
        <ul className={styles.candidates}>
          <li>
            <button type="button" onClick={() => void submit(t("reply.clarify.yes"))}>
              {t("reply.clarify.yes")}
            </button>
          </li>
          <li>
            <button type="button" onClick={() => void submit(t("reply.clarify.no"))}>
              {t("reply.clarify.no")}
            </button>
          </li>
        </ul>
      );
    }
    return (
      <ul className={styles.candidates}>
        {c.candidates.map((app) => (
          <li key={app.id}>
            <button type="button" onClick={() => void submit(app.displayName)}>
              {applicationName(t, app)}
            </button>
          </li>
        ))}
      </ul>
    );
  }
  // Without the catalog in understanding (older path): re-ask by name.
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

const STEP_MARK: Record<string, string> = {
  pending: "○",
  running: "◌",
  completed: "✓",
  needsConfirmation: "…",
  unresolved: "!",
  failed: "✕",
  skipped: "–",
  cancelled: "–",
};

/** A bounded plan and each step's state (no hidden reasoning, no ids). */
export function PlanCard({ plan, live = false }: { plan: PlanReport; live?: boolean }) {
  const { t } = useI18n();
  return (
    <ol
      className={styles.plan}
      aria-label={live ? t("plan.liveLabel") : t("plan.label")}
      data-live={live || undefined}
    >
      {plan.steps.map((step, i) => (
        <li key={i} className={styles.planStep} data-status={step.status}>
          <span className={styles.planMark} aria-hidden="true">
            {STEP_MARK[step.status]}
          </span>
          <span>{stepLabel(t, step)}</span>
          <span className={styles.srOnly}>{t(`plan.status.${step.status}`)}</span>
        </li>
      ))}
    </ol>
  );
}

/** The session conversation. SERSHI's replies are announced to screen readers. */
export function Transcript({
  messages,
  livePlan = null,
}: {
  messages: Message[];
  livePlan?: PlanReport | null;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const i18n = useI18n();
  const { t, format } = i18n;
  // Following the latest turn, unless the user scrolled up to read.
  const following = useRef(true);
  const count = messages.length + (livePlan ? 1 : 0);
  const [atEnd, setAtEnd] = useState(true);
  const [seen, setSeen] = useState(count);
  const unread = !atEnd && count > seen;

  const toLatest = useCallback((smooth: boolean) => {
    const reduce = motionReduced();
    following.current = true;
    end.current?.scrollIntoView({
      behavior: smooth && !reduce ? "smooth" : "auto",
      block: "end",
    });
  }, []);

  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    const near = el.scrollHeight - el.scrollTop - el.clientHeight <= NEAR_END_PX;
    following.current = near;
    setAtEnd(near);
    if (near) setSeen(count);
  };

  // New turns scroll into view only while the user follows the latest;
  // someone reading older turns is never snapped back to the bottom (the
  // "Jump to latest" button appears instead).
  useLayoutEffect(() => {
    if (following.current) toLatest(true);
  }, [count, toLatest]);

  useEffect(() => {
    toLatest(false);
  }, [toLatest]);

  return (
    <div className={styles.frame}>
      <div
        ref={scroller}
        className={styles.scroller}
        onScroll={onScroll}
        // Keyboard users scroll the conversation with the arrow keys.
        tabIndex={0}
        role="region"
        aria-label={t("home.conversationLabel")}
      >
        <ol className={styles.list} aria-live="polite" aria-relevant="additions">
          {messages.map((m, index) => {
            if (m.role === "user") {
              return (
                <li key={m.id} className={styles.message} data-role="user" data-via={m.via}>
                  {m.via === "voice" && (
                    <p className={styles.via}>
                      <MicIcon />
                      {m.firstHeard ? t("transcript.heardAgain") : t("transcript.youSaid")}
                    </p>
                  )}
                  <p className={styles.user}>{m.text}</p>
                  {m.firstHeard && (
                    <p className={styles.firstHeard}>
                      {t("transcript.firstHeard", { text: m.firstHeard })}
                    </p>
                  )}
                </li>
              );
            }
            const status = statusOf(m.reply);
            const outcome = m.reply.kind === "outcome" ? m.reply.outcome : null;
            return (
              <li key={m.id} className={styles.message} data-role="sershi" data-status={status}>
                {outcome?.understood && (
                  <p className={styles.understood}>
                    {t("reply.understood.label")}{" "}
                    <strong>{understoodAction(t, outcome.understood)}</strong>
                  </p>
                )}
                {outcome?.plan && <PlanCard plan={outcome.plan} />}
                <p className={styles.reply}>{replyText(i18n, m.reply)}</p>
                <Candidates reply={m.reply} latest={index === messages.length - 1} />
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
          {livePlan && (
            <li className={styles.message} data-role="sershi" data-status="running">
              <PlanCard plan={livePlan} live />
            </li>
          )}
        </ol>
        <div ref={end} />
      </div>
      {unread && (
        <button
          type="button"
          className={styles.jump}
          onClick={() => {
            toLatest(true);
          }}
        >
          <ArrowDownIcon />
          {t("home.jumpToLatest")}
        </button>
      )}
    </div>
  );
}
