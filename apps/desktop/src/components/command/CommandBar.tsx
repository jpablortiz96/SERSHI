import { useEffect, useRef, useState, type KeyboardEvent, type SyntheticEvent } from "react";

import { useI18n } from "../../i18n";
import { desktopRuntime, sershi } from "../../ipc";
import { useAssistantStore } from "../../state/assistant";
import { useConversation } from "../../state/conversation";
import { useVoice } from "../../state/voice";
import { ArrowUpIcon, MicIcon, StopIcon, WaveIcon } from "../shell/icons";
import styles from "./CommandBar.module.css";
import { VoicePanel } from "./VoicePanel";
import { VoiceSessionBar } from "./VoiceSessionBar";

/** Mirrors MAX_COMMAND_CHARS in sershi-core. */
const MAX_CHARS = 1000;
/** How long the accepted command lingers as it lifts away (matches CSS). */
export const ACCEPTED_MS = 520;

/**
 * The primary input. Keyboard-first: "/" focuses from anywhere, Enter sends,
 * ↑ recalls the previous command, Escape clears or dismisses.
 *
 * Push-to-talk: the microphone button opens the microphone (click again to
 * stop and send; Escape cancels). The button's pressed state, the placeholder
 * and the assistant state all say when SERSHI is listening — it never
 * listens otherwise. Voice is additive: typing always works.
 *
 * Voice session (Gate 4.1): the wave button starts a hands-free
 * conversation — SERSHI listens again after each reply until the user says
 * goodbye, presses End or stays silent. The session bar shows it whenever
 * it is active. While SERSHI speaks or works, the microphone button
 * interrupts it (barge-in).
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
  // Behaviour follows the real state, never a Developer Mode preview.
  const state = useAssistantStore((s) => s.snapshot.state);
  const voiceStatus = useVoice((s) => s.status);
  const toggleCapture = useVoice((s) => s.toggleCapture);
  const cancelCapture = useVoice((s) => s.cancelCapture);
  const stopSpeaking = useVoice((s) => s.stopSpeaking);
  const session = useVoice((s) => s.session);
  const startSession = useVoice((s) => s.startSession);
  const stopSession = useVoice((s) => s.stopSession);
  const inSession = session !== null;
  const listening = state === "listening";
  const transcribing = state === "transcribing";
  const speaking = state === "speaking";
  const voiceAvailable = desktopRuntime && voiceStatus?.supported === true;
  const listeningRef = useRef(listening);
  useEffect(() => {
    listeningRef.current = listening;
  }, [listening]);

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
      // Escape anywhere cancels listening (the input handles its own).
      if (e.key === "Escape" && listeningRef.current && target !== input.current) {
        e.preventDefault();
        cancelCapture();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.clearTimeout(acceptedTimer.current);
    };
  }, [cancelCapture]);

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
      if (listening) cancelCapture();
      else if (text) setText("");
      else sershi.dismissAssistant().catch(() => undefined);
    }
  };

  const interrupting = speaking || state === "thinking" || state === "executing";
  const micLabel = listening
    ? t("command.voiceStop")
    : transcribing
      ? t("command.voiceBusy")
      : !voiceAvailable
        ? t("command.voiceUnavailable")
        : interrupting
          ? t("command.interrupt")
          : t("command.voice");
  const preparing = useVoice((s) => s.preparing);
  const placeholder = listening
    ? t("command.listeningPlaceholder")
    : transcribing && preparing
      ? t("command.preparingPlaceholder")
      : transcribing
        ? t("command.transcribingPlaceholder")
        : t("command.placeholder");

  return (
    <form className={styles.form} onSubmit={send}>
      <VoicePanel />
      <VoiceSessionBar />
      <div
        className={styles.bar}
        data-pending={pending || transcribing || undefined}
        data-typing={text.trim() ? true : undefined}
        data-voice={listening ? "listening" : undefined}
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
          placeholder={placeholder}
          aria-label={t("command.label")}
          autoComplete="off"
          spellCheck={false}
          autoFocus
        />
        {speaking && (
          <button
            type="button"
            className={styles.icon}
            onClick={stopSpeaking}
            aria-label={t("command.stopSpeaking")}
            data-tip={t("command.stopSpeaking")}
          >
            <StopIcon />
          </button>
        )}
        <button
          type="button"
          className={styles.icon}
          data-session={inSession || undefined}
          disabled={!voiceAvailable}
          aria-pressed={inSession}
          aria-label={inSession ? t("command.sessionStop") : t("command.session")}
          data-tip={inSession ? t("command.sessionStop") : t("command.sessionTip")}
          onClick={() => {
            if (inSession) stopSession();
            else void startSession();
          }}
        >
          <WaveIcon />
        </button>
        <button
          type="button"
          className={styles.icon}
          data-mic={listening ? "listening" : transcribing ? "transcribing" : undefined}
          disabled={!voiceAvailable || transcribing}
          aria-pressed={listening}
          aria-label={micLabel}
          data-tip={listening ? t("command.voiceStop") : t("command.voiceTip")}
          onClick={() => {
            void toggleCapture(listening);
          }}
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
