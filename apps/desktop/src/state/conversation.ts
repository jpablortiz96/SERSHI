/**
 * Session conversation. Held in memory only and discarded on exit — the core
 * never stores what the user typed (see docs/MEMORY.md, "Session memory").
 *
 * SERSHI's replies are kept as structured outcomes, not sentences, so they are
 * phrased at render time in the current interface language and re-render
 * when the user switches language.
 */
import type { CommandOutcome } from "@sershi/contracts";
import { create } from "zustand";

import { desktopRuntime, sershi } from "../ipc";

export type Reply =
  | { kind: "outcome"; outcome: CommandOutcome }
  /** Browser preview: no core to answer. */
  | { kind: "offline" }
  /** The IPC call itself failed. */
  | { kind: "unreachable" };

export type Message =
  { id: number; role: "user"; text: string } | { id: number; role: "sershi"; reply: Reply };

interface ConversationStore {
  messages: Message[];
  pending: boolean;
  submit: (text: string) => Promise<void>;
  /** Adds a reply that did not come from `submit` (a decision on the confirmation surface). */
  addReply: (reply: Reply) => void;
  clear: () => void;
}

/** Messages kept in view; older ones are dropped from memory. */
const MAX_MESSAGES = 40;

let nextId = 1;

type NewMessage = { role: "user"; text: string } | { role: "sershi"; reply: Reply };

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
    submit: async (raw) => {
      const text = raw.trim();
      if (!text || get().pending) return;
      push({ role: "user", text });

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
      } catch {
        push({ role: "sershi", reply: { kind: "unreachable" } });
      } finally {
        set({ pending: false });
      }
    },
  };
});
