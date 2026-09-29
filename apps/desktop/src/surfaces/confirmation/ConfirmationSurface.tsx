/**
 * The trusted confirmation surface — its own window, entry point and IPC
 * capability. It renders one confirmation the core assigned to it and
 * returns the human's decision; nothing else.
 *
 * Every string comes from SERSHI's own locale resources plus the core's
 * trusted, resolved subject. Nothing from the request text, a tool or a
 * model is displayed.
 *
 * Accidental-input resistance:
 * - Cancel has focus when the window opens; Enter on it cancels.
 * - Escape cancels at any time.
 * - Approve stays inactive for a short arming delay after the window has
 *   focus and content ({@link ARMING_MS}), and only accepts a click whose
 *   press began on it after arming, or a fresh (non-repeated) key press made
 *   while it had focus. A click or held key that started before the window
 *   appeared cannot approve.
 */
import type { ConfirmationChoice, ConfirmationRequest } from "@sershi/contracts";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";

import { Core } from "../../components/core/Core";
import { ApprovalMark, CloseIcon } from "../../components/shell/icons";
import { useI18n, type Translate } from "../../i18n";
import { applicationName, riskLabel, toolName } from "../../i18n/domain";
import { confirmationSurface, desktopRuntime } from "../../ipc/confirmation";
import styles from "./ConfirmationSurface.module.css";

/** How long Approve ignores input after the surface is shown and focused. */
export const ARMING_MS = 600;

/** Browser preview only (`confirmation.html?preview`): never sent anywhere. */
function previewRequest(): ConfirmationRequest {
  return {
    id: "0".repeat(32),
    toolId: "system.close_application",
    action: "closeApplication",
    subject: {
      kind: "application",
      application: { id: "spotify", displayName: "Spotify", source: "packagedApp" },
    },
    risk: "sensitive",
    level: "standard",
    reason: "permissionUndecided",
    canRemember: false,
    expiresAtMs: Date.now() + 60_000,
  };
}

function copy(t: Translate, request: ConfirmationRequest) {
  const tool = toolName(t, request.toolId);
  const subject =
    request.subject?.kind === "application"
      ? applicationName(t, request.subject.application)
      : null;
  if (request.action === "closeApplication" && subject) {
    return {
      title: t("confirm.closeApplication.title", { app: subject }),
      body: t("confirm.closeApplication.body", { app: subject }),
      risk: t("confirm.closeApplication.risk"),
      confirm: t("confirm.closeApplication.confirm", { app: subject }),
    };
  }
  return {
    title: t("confirm.runTool.title", { tool }),
    body: t("confirm.runTool.body", { tool }),
    risk: request.level === "highRisk" ? t("confirm.reason.highRisk") : null,
    confirm: t("confirm.runTool.confirm"),
  };
}

function secondsLeft(request: ConfirmationRequest) {
  return Math.max(0, Math.ceil((request.expiresAtMs - Date.now()) / 1000));
}

type Phase =
  | { kind: "loading" }
  | { kind: "ready"; request: ConfirmationRequest; preview: boolean }
  | { kind: "deciding"; request: ConfirmationRequest }
  | { kind: "expired" }
  | { kind: "unavailable" };

function initialPhase(): Phase {
  if (desktopRuntime) return { kind: "loading" };
  const preview = new URLSearchParams(window.location.search).has("preview");
  return preview
    ? { kind: "ready", request: previewRequest(), preview: true }
    : { kind: "unavailable" };
}

export function ConfirmationSurface() {
  const { t } = useI18n();
  const [phase, setPhase] = useState<Phase>(initialPhase);

  useEffect(() => {
    if (!desktopRuntime) return;
    let live = true;
    confirmationSurface
      .getContext()
      .then((request) => {
        if (live) setPhase({ kind: "ready", request, preview: false });
      })
      .catch(() => {
        if (live) setPhase({ kind: "unavailable" });
      });
    return () => {
      live = false;
    };
  }, []);

  const decide = useCallback(
    (request: ConfirmationRequest, preview: boolean, choice: ConfirmationChoice) => {
      if (preview) {
        setPhase({ kind: "unavailable" });
        return;
      }
      setPhase({ kind: "deciding", request });
      // Rust closes this window once the decision is applied.
      confirmationSurface.decide(request.id, choice).catch(() => {
        setPhase({ kind: "unavailable" });
      });
    },
    [],
  );

  const onExpire = useCallback(() => {
    // Nothing to send: the core expires it and closes this window.
    setPhase({ kind: "expired" });
  }, []);

  return (
    <div className={styles.surface} data-state="awaitingConfirmation">
      {phase.kind === "ready" || phase.kind === "deciding" ? (
        <Prompt
          key={phase.request.id}
          request={phase.request}
          busy={phase.kind === "deciding"}
          onDecide={(choice) => {
            if (phase.kind === "ready") decide(phase.request, phase.preview, choice);
          }}
          onExpire={onExpire}
        />
      ) : (
        <Notice phase={phase.kind} t={t} />
      )}
    </div>
  );
}

function Notice({ phase, t }: { phase: "loading" | "expired" | "unavailable"; t: Translate }) {
  if (phase === "loading") return <p className={styles.notice} aria-busy="true" />;
  return (
    <p className={styles.notice} role="status">
      {phase === "expired" ? t("confirm.expired") : t("confirm.unavailable")}
    </p>
  );
}

interface PromptProps {
  request: ConfirmationRequest;
  busy: boolean;
  onDecide: (choice: ConfirmationChoice) => void;
  onExpire: () => void;
}

function Prompt({ request, busy, onDecide, onExpire }: PromptProps) {
  const { t } = useI18n();
  const text = copy(t, request);
  const cancel = useRef<HTMLButtonElement>(null);
  const [armed, setArmed] = useState(false);
  const [left, setLeft] = useState(() => secondsLeft(request));
  // Measured once when the prompt opens (the meter animates the rest).
  const [total] = useState(() => Math.max(1, request.expiresAtMs - Date.now()));
  /** A press that began on Approve after arming (pointer or fresh key). */
  const intent = useRef(false);

  useEffect(() => {
    cancel.current?.focus();
  }, []);

  // Arm only once the window is focused, then after a short, steady delay.
  useEffect(() => {
    let timer: number | undefined;
    const start = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        setArmed(true);
      }, ARMING_MS);
    };
    const disarm = () => {
      window.clearTimeout(timer);
      intent.current = false;
      setArmed(false);
    };
    if (document.hasFocus()) start();
    window.addEventListener("focus", start);
    window.addEventListener("blur", disarm);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("focus", start);
      window.removeEventListener("blur", disarm);
    };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const remaining = secondsLeft(request);
      setLeft(remaining);
      if (remaining <= 0) onExpire();
    }, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [request, onExpire]);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && !busy) {
      e.preventDefault();
      onDecide("cancel");
    }
  };

  const approveKey = (e: KeyboardEvent) => {
    if (e.key !== "Enter" && e.key !== " ") return;
    if (!armed || e.repeat) {
      // A held key from before the window appeared, or auto-repeat.
      e.preventDefault();
      intent.current = false;
      return;
    }
    intent.current = true;
  };

  const approve = () => {
    const deliberate = armed && intent.current;
    intent.current = false;
    if (deliberate && !busy) onDecide("approve");
  };

  return (
    <div
      className={styles.dialog}
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-title"
      aria-describedby="confirm-body"
      data-level={request.level}
      onKeyDown={onKeyDown}
    >
      <div className={styles.header}>
        <span className={styles.mark} aria-hidden="true">
          <Core state="awaitingConfirmation" variant="compact" decorative />
        </span>
        <span className={styles.brand}>SERSHI</span>
        {/* SERSHI's approval surface marker (visual consistency only; not an
            OS-level identity or security claim). */}
        <span className={styles.trust}>
          <ApprovalMark />
          {t("confirm.trust")}
        </span>
        <span className={styles.risk} data-risk={request.risk}>
          {riskLabel(t, request.risk)}
        </span>
        <button
          type="button"
          className={styles.close}
          aria-label={t("confirm.dismiss")}
          data-tip={t("confirm.dismiss")}
          disabled={busy}
          onClick={() => {
            onDecide("cancel");
          }}
        >
          <CloseIcon />
        </button>
      </div>

      <p className={`t-label ${styles.label}`}>{t("confirm.label")}</p>
      <h1 id="confirm-title" className={styles.title}>
        {text.title}
      </h1>
      <div id="confirm-body" className={styles.body}>
        <p>{text.body}</p>
        {text.risk && <p className={styles.caution}>{text.risk}</p>}
        <p className={styles.reason}>{t(`confirm.reason.${request.reason}`)}</p>
      </div>

      <div className={styles.expiry} aria-hidden="true">
        <div className={styles.meter}>
          <i style={{ "--expiry-ms": `${total}ms` } as CSSProperties} />
        </div>
        <span className={styles.countdown}>{t("confirm.expires", { seconds: left })}</span>
      </div>

      <div className={styles.actions}>
        <button
          ref={cancel}
          type="button"
          className={styles.cancel}
          disabled={busy}
          onClick={() => {
            onDecide("cancel");
          }}
        >
          {t("confirm.cancel")}
        </button>
        <button
          type="button"
          className={styles.confirm}
          disabled={busy}
          aria-disabled={!armed}
          data-armed={armed}
          onPointerDown={() => {
            intent.current = armed;
          }}
          onKeyDown={approveKey}
          onClick={approve}
        >
          {text.confirm}
        </button>
      </div>
    </div>
  );
}
