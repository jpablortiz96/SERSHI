/**
 * Session conversation. Held in memory only and discarded on exit — the core
 * never stores what the user typed (see docs/MEMORY.md, "Session memory").
 */
import type { CommandStatus, ToolId } from "@sershi/contracts";
import { create } from "zustand";

import { desktopRuntime, IpcFailure, sershi } from "../ipc";

export interface Message {
  id: number;
  role: "user" | "sershi";
  text: string;
  status?: CommandStatus | "offline";
  toolId?: ToolId | null;
  durationMs?: number | null;
}

interface ConversationStore {
  messages: Message[];
  pending: boolean;
  submit: (text: string) => Promise<void>;
  clear: () => void;
}

/** Messages kept in view; older ones are dropped from memory. */
const MAX_MESSAGES = 40;

let nextId = 1;

export const useConversation = create<ConversationStore>((set, get) => {
  const push = (message: Omit<Message, "id">) => {
    set((s) => ({ messages: [...s.messages, { ...message, id: nextId++ }].slice(-MAX_MESSAGES) }));
  };

  return {
    messages: [],
    pending: false,
    clear: () => {
      set({ messages: [] });
    },
    submit: async (raw) => {
      const text = raw.trim();
      if (!text || get().pending) return;
      push({ role: "user", text });

      if (!desktopRuntime) {
        push({
          role: "sershi",
          status: "offline",
          text: "I'm running as a browser preview, so my core isn't connected. Launch the desktop app (pnpm dev) to talk to me.",
        });
        return;
      }

      set({ pending: true });
      try {
        const outcome = await sershi.submitCommand(text);
        push({
          role: "sershi",
          text: outcome.reply,
          status: outcome.status,
          toolId: outcome.toolId,
          durationMs: outcome.durationMs,
        });
      } catch (error) {
        push({
          role: "sershi",
          status: "failed",
          text:
            error instanceof IpcFailure ? error.message : "Something went wrong reaching my core.",
        });
      } finally {
        set({ pending: false });
      }
    },
  };
});
