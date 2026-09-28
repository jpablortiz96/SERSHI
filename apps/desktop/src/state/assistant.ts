/**
 * Mirror of the core's assistant state. Rust owns the state machine; this
 * store only holds the latest snapshot it broadcast, so the companion and the
 * Command Center always agree. The one local addition is `localPreview`, a
 * design tool for the browser preview where no core is running.
 */
import { isAssistantState, type AssistantSnapshot, type AssistantState } from "@sershi/contracts";
import { create } from "zustand";

import { desktopRuntime, sershi } from "../ipc";

export type Connection = "connecting" | "live" | "browserPreview" | "failed";

interface AssistantStore {
  connection: Connection;
  snapshot: AssistantSnapshot;
  localPreview: AssistantState | null;
  receive: (snapshot: AssistantSnapshot) => void;
  setConnection: (connection: Connection) => void;
  setLocalPreview: (state: AssistantState | null) => void;
}

const initialSnapshot: AssistantSnapshot = { state: "idle", previewState: null, revision: -1 };

export const useAssistantStore = create<AssistantStore>((set) => ({
  connection: desktopRuntime ? "connecting" : "browserPreview",
  snapshot: initialSnapshot,
  localPreview: null,
  // Events and the initial fetch can race; the newest revision wins.
  receive: (snapshot) => {
    set((s) => (snapshot.revision >= s.snapshot.revision ? { snapshot } : s));
  },
  setConnection: (connection) => {
    set({ connection });
  },
  setLocalPreview: (localPreview) => {
    set({ localPreview });
  },
}));

/** The state surfaces should render (preview overrides included). */
export function selectDisplayState(s: AssistantStore): AssistantState {
  return s.localPreview ?? s.snapshot.previewState ?? s.snapshot.state;
}

export const useDisplayState = () => useAssistantStore(selectDisplayState);

/** Connects the store to the core. Returns a cleanup function. */
export function connectAssistant(): () => void {
  const store = useAssistantStore.getState();
  if (!desktopRuntime) {
    // Browser preview: allow ?preview=<state> for design review.
    const requested = new URLSearchParams(window.location.search).get("preview");
    if (isAssistantState(requested)) store.setLocalPreview(requested);
    return () => undefined;
  }
  const unsubscribe = sershi.onAssistantState(store.receive);
  sershi
    .getAssistantSnapshot()
    .then((snapshot) => {
      store.receive(snapshot);
      store.setConnection("live");
    })
    .catch(() => {
      store.setConnection("failed");
    });
  return unsubscribe;
}
