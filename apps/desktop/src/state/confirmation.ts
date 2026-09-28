/**
 * The confirmation currently shown to the user.
 *
 * This store only *displays* a request the core created. Approving sends
 * `{ confirmationId, toolId, approved }`; the core verifies the id is known,
 * pending, unexpired and for the same tool, and runs the call it stored. No
 * state here can authorize anything.
 */
import type { ConfirmationRequest } from "@sershi/contracts";
import { create } from "zustand";

import { desktopRuntime, sershi } from "../ipc";
import { useConversation } from "./conversation";

interface ConfirmationStore {
  pending: ConfirmationRequest | null;
  /** Developer Mode preview: rendered, but decisions do nothing. */
  preview: boolean;
  deciding: boolean;
  show: (request: ConfirmationRequest) => void;
  showPreview: (request: ConfirmationRequest) => void;
  clear: () => void;
  decide: (approved: boolean) => Promise<void>;
  /** Called when the request's expiry passes while it is still shown. */
  expire: () => void;
}

export const useConfirmation = create<ConfirmationStore>((set, get) => ({
  pending: null,
  preview: false,
  deciding: false,
  show: (pending) => {
    set({ pending, preview: false, deciding: false });
  },
  showPreview: (pending) => {
    set({ pending, preview: true, deciding: false });
  },
  clear: () => {
    set({ pending: null, preview: false, deciding: false });
  },
  decide: async (approved) => {
    const { pending, preview, deciding } = get();
    if (!pending || deciding) return;
    if (preview || !desktopRuntime) {
      get().clear();
      return;
    }
    set({ deciding: true });
    const { addReply } = useConversation.getState();
    try {
      const outcome = await sershi.decideConfirmation(pending, approved);
      addReply({ kind: "outcome", outcome });
    } catch {
      addReply({ kind: "unreachable" });
    } finally {
      get().clear();
    }
  },
  expire: () => {
    const { pending, preview } = get();
    if (!pending) return;
    get().clear();
    if (!preview) useConversation.getState().addReply({ kind: "expired" });
  },
}));

/** Restores a confirmation still pending in the core (e.g. after a reload). */
export function restorePendingConfirmation(): void {
  if (!desktopRuntime) return;
  sershi
    .getPendingConfirmation()
    .then((request) => {
      if (request && request.expiresAtMs > Date.now()) useConfirmation.getState().show(request);
    })
    .catch(() => undefined);
}
