import { useEffect, useRef, useState, type KeyboardEvent, type SyntheticEvent } from "react";

import { sershi } from "../../ipc";
import { useConversation } from "../../state/conversation";
import { ArrowUpIcon, MicIcon } from "../shell/icons";
import styles from "./CommandBar.module.css";

/** Mirrors MAX_COMMAND_CHARS in sershi-core. */
const MAX_CHARS = 1000;

/**
 * The primary input. Keyboard-first: "/" focuses from anywhere, Enter sends,
 * ↑ recalls the previous command, Escape clears or dismisses.
 */
export function CommandBar() {
  const [text, setText] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const history = useRef<string[]>([]);
  const { submit, pending } = useConversation();

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
    };
  }, []);

  const send = (e?: SyntheticEvent) => {
    e?.preventDefault();
    const value = text.trim();
    if (!value || pending) return;
    history.current = [...history.current.filter((h) => h !== value), value].slice(-20);
    setText("");
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
      <div className={styles.bar} data-pending={pending || undefined}>
        <span className={styles.signal} aria-hidden="true" />
        <input
          ref={input}
          className={styles.input}
          value={text}
          maxLength={MAX_CHARS}
          onChange={(e) => {
            setText(e.target.value);
          }}
          onKeyDown={onKeyDown}
          placeholder="Ask SERSHI or type a command"
          aria-label="Command"
          autoComplete="off"
          spellCheck={false}
          autoFocus
        />
        <button
          type="button"
          className={styles.icon}
          disabled
          aria-label="Voice input, coming in v0.3"
          data-tip="Voice arrives in v0.3"
        >
          <MicIcon />
        </button>
        <button
          type="submit"
          className={styles.send}
          disabled={!text.trim() || pending}
          aria-label="Send"
        >
          <ArrowUpIcon />
        </button>
      </div>
      <p className={styles.hints} aria-hidden="true">
        <kbd>Enter</kbd> send <span>·</span> <kbd>/</kbd> focus <span>·</span> <kbd>↑</kbd> last
        command <span>·</span> <kbd>Esc</kbd> dismiss
      </p>
    </form>
  );
}
