import { useEffect, useRef, useState, type KeyboardEvent, type SyntheticEvent } from "react";

import { useI18n } from "../../i18n";
import { sershi } from "../../ipc";
import { useConversation } from "../../state/conversation";
import { ArrowUpIcon, MicIcon } from "../shell/icons";
import styles from "./CommandBar.module.css";

/** Mirrors MAX_COMMAND_CHARS in sershi-core. */
const MAX_CHARS = 1000;
/** How long the accepted command lingers as it lifts away (matches CSS). */
export const ACCEPTED_MS = 520;

/**
 * The primary input. Keyboard-first: "/" focuses from anywhere, Enter sends,
 * ↑ recalls the previous command, Escape clears or dismisses.
 */
export function CommandBar() {
  const [text, setText] = useState("");
  /** The command just sent, shown lifting away while the input clears. */
  const [accepted, setAccepted] = useState<{ id: number; text: string } | null>(null);
  const acceptedTimer = useRef<number | undefined>(undefined);
  const input = useRef<HTMLInputElement>(null);
  const history = useRef<string[]>([]);
  const { submit, pending } = useConversation();
  const { t } = useI18n();

  // Summon (companion, tray, global shortcut) focuses the command input.
  useEffect(
    () =>
      sershi.onFocusCommand(() => {
        input.current?.focus();
      }),
    [],
  );

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing = target?.closest("input, textarea, [contenteditable]");
      if (e.key === "/" && !typing) {
        e.preventDefault();
        input.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.clearTimeout(acceptedTimer.current);
    };
  }, []);

  const send = (e?: SyntheticEvent) => {
    e?.preventDefault();
    const value = text.trim();
    if (!value || pending) return;
    history.current = [...history.current.filter((h) => h !== value), value].slice(-20);
    setText("");
    // Acknowledge the request visibly before the assistant state takes over.
    setAccepted({ id: Date.now(), text: value });
    window.clearTimeout(acceptedTimer.current);
    acceptedTimer.current = window.setTimeout(() => {
      setAccepted(null);
    }, ACCEPTED_MS);
    void submit(value);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowUp" && !text) {
      const previous = history.current.at(-1);
      if (previous) {
        e.preventDefault();
        setText(previous);
      }
    } else if (e.key === "Escape") {
      if (text) setText("");
      else sershi.dismissAssistant().catch(() => undefined);
    }
  };

  return (
    <form className={styles.form} onSubmit={send}>
      <div
        className={styles.bar}
        data-pending={pending || undefined}
        data-typing={text.trim() ? true : undefined}
      >
        <span className={styles.signal} aria-hidden="true" />
        {accepted && (
          <span key={accepted.id} className={styles.accepted} aria-hidden="true">
            {accepted.text}
          </span>
        )}
        <input
          ref={input}
          className={styles.input}
          value={text}
          maxLength={MAX_CHARS}
          onChange={(e) => {
            setText(e.target.value);
          }}
          onKeyDown={onKeyDown}
          placeholder={t("command.placeholder")}
          aria-label={t("command.label")}
          autoComplete="off"
          spellCheck={false}
          autoFocus
        />
        <button
          type="button"
          className={styles.icon}
          disabled
          aria-label={t("command.voice")}
          data-tip={t("command.voiceTip")}
        >
          <MicIcon />
        </button>
        <button
          type="submit"
          className={styles.send}
          disabled={!text.trim() || pending}
          aria-label={t("command.send")}
        >
          <ArrowUpIcon />
        </button>
      </div>
      <p className={styles.hints} aria-hidden="true">
        <span>
          <kbd>{t("command.keyEnter")}</kbd> {t("command.hintSend")}
        </span>
        <span>
          <kbd>/</kbd> {t("command.hintFocus")}
        </span>
        <span>
          <kbd>↑</kbd> {t("command.hintRecall")}
        </span>
        <span>
          <kbd>{t("command.keyEscape")}</kbd> {t("command.hintDismiss")}
        </span>
      </p>
    </form>
  );
}
