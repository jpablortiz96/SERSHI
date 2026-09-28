import type { ConfirmationRequest } from "@sershi/contracts";
import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";

import { useI18n, type Translate } from "../../i18n";
import { applicationName, riskLabel, toolName } from "../../i18n/domain";
import { useConfirmation } from "../../state/confirmation";
import styles from "./ConfirmationDialog.module.css";

/** Every string comes from SERSHI's own resources plus the core's trusted subject. */
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
    risk: null,
    confirm: t("confirm.runTool.confirm"),
  };
}

function secondsLeft(request: ConfirmationRequest) {
  return Math.max(0, Math.ceil((request.expiresAtMs - Date.now()) / 1000));
}

/**
 * Trusted approval UI. Initial focus is on Cancel, so a stray Enter never
 * approves; Escape cancels; focus is trapped inside and returned afterwards.
 */
export function ConfirmationDialog() {
  const { pending, preview, deciding, decide, expire } = useConfirmation();
  if (!pending) return null;
  return (
    <Dialog
      key={pending.id}
      request={pending}
      preview={preview}
      deciding={deciding}
      onDecide={(approved) => void decide(approved)}
      onExpire={expire}
    />
  );
}

interface DialogProps {
  request: ConfirmationRequest;
  preview: boolean;
  deciding: boolean;
  onDecide: (approved: boolean) => void;
  onExpire: () => void;
}

function Dialog({ request, preview, deciding, onDecide, onExpire }: DialogProps) {
  const { t } = useI18n();
  const dialog = useRef<HTMLDivElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const [left, setLeft] = useState(() => secondsLeft(request));
  const text = copy(t, request);
  // Measured once when the dialog opens (the meter animates the rest).
  const [total] = useState(() => Math.max(1, request.expiresAtMs - Date.now()));

  // Focus Cancel on open; give focus back to whatever had it on close.
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    cancel.current?.focus();
    return () => previous?.focus();
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
    if (e.key === "Escape") {
      e.preventDefault();
      onDecide(false);
    } else if (e.key === "Tab") {
      const focusable = dialog.current?.querySelectorAll<HTMLElement>("button:not(:disabled)");
      if (!focusable || focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    }
  };

  return (
    <div className={styles.backdrop}>
      <div
        ref={dialog}
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-body"
        data-state="awaitingConfirmation"
        onKeyDown={onKeyDown}
      >
        <div className={styles.header}>
          <span className={styles.glyph} aria-hidden="true" />
          <span className="t-label">{t("confirm.label")}</span>
          <span className={styles.risk} data-risk={request.risk}>
            {riskLabel(t, request.risk)}
          </span>
        </div>

        <h2 id="confirm-title" className={styles.title}>
          {text.title}
        </h2>
        <div id="confirm-body" className={styles.body}>
          <p>{text.body}</p>
          {text.risk && <p className={styles.caution}>{text.risk}</p>}
          <p className={styles.reason}>{t(`confirm.reason.${request.reason}`)}</p>
          {preview && <p className={styles.reason}>{t("confirm.preview")}</p>}
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
            disabled={deciding}
            onClick={() => {
              onDecide(false);
            }}
          >
            {t("confirm.cancel")}
          </button>
          <button
            type="button"
            className={styles.confirm}
            disabled={deciding}
            onClick={() => {
              onDecide(true);
            }}
          >
            {text.confirm}
          </button>
        </div>
      </div>
    </div>
  );
}
