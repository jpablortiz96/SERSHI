/**
 * Session conversation. Held in memory only and discarded on exit — the core
 * never stores what the user typed or said (see docs/MEMORY.md, "Session
 * memory"). Spoken requests appear here as "You said …" exactly like typed
 * ones; they were submitted by the core through the same command path.
 *
 * SERSHI's replies are kept as structured outcomes, not sentences, so they are
 * phrased at render time in the current interface language and re-render
 * when the user switches language.
 */
import type { CommandOutcome, VoiceFailure } from "@sershi/contracts";
import { create } from "zustand";

import { loadPreferences } from "../i18n/preferences";
import { desktopRuntime, sershi } from "../ipc";
import { clearSpeechExpectation, expectSpeech, speakOutcome, stopSpeaking } from "./speech";

export type Reply =
  | { kind: "outcome"; outcome: CommandOutcome }
  /** Browser preview: no core to answer. */
  | { kind: "offline" }
  /** The IPC call itself failed. */
  | { kind: "unreachable" }
  /** A voice interaction ended without a command (nothing ran). */
  | { kind: "voice"; notice: "noSpeech" | "unclear" | VoiceFailure };

/** How the user asked: typed, or spoken (then transcribed locally). */
export type Via = "typed" | "voice";

export type Message =
  | { id: number; role: "user"; text: string; via: Via }
  | { id: number; role: "sershi"; reply: Reply };

interface ConversationStore {
  messages: Message[];
  pending: boolean;
  submit: (text: string) => Promise<void>;
  /** Adds a reply that did not come from `submit` (a decision on the confirmation surface). */
  addReply: (reply: Reply) => void;
  /** Shows what SERSHI heard; the core has already submitted it. */
  addHeard: (text: string) => void;
  clear: () => void;
}

/** Messages kept in view; older ones are dropped from memory. */
const MAX_MESSAGES = 40;

let nextId = 1;

type NewMessage = { role: "user"; text: string; via: Via } | { role: "sershi"; reply: Reply };

export const useConversation = create<ConversationStore>((set, get) => {
  const push = (message: NewMessage) => {
    set((s) => ({ messages: [...s.messages, { ...message, id: nextId++ }].slice(-MAX_MESSAGES) }));
  };

  return {
    messages: [],
    pending: false,
    clear: () => {
      set({ messages: [] });
    },
    addReply: (reply) => {
      push({ role: "sershi", reply });
    },
    addHeard: (text) => {
      push({ role: "user", text, via: "voice" });
    },
    submit: async (raw) => {
      const text = raw.trim();
      if (!text || get().pending) return;
      push({ role: "user", text, via: "typed" });
      // Typing takes over from a spoken reply (the core also stops it).
      clearSpeechExpectation();
      stopSpeaking();

      if (!desktopRuntime) {
        push({ role: "sershi", reply: { kind: "offline" } });
        return;
      }

      set({ pending: true });
      try {
        // A new request cancels any pending approval (in the core). If this
        // one needs approval, the core opens the confirmation window; this
        // surface only learns that approval is pending.
        const outcome = await sershi.submitCommand(text);
        push({ role: "sershi", reply: { kind: "outcome", outcome } });
        if (loadPreferences().speakTypedResponses) {
          expectSpeech();
          speakOutcome(outcome);
        }
      } catch {
        push({ role: "sershi", reply: { kind: "unreachable" } });
      } finally {
        set({ pending: false });
      }
    },
  };
});
