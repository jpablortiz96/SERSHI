/**
 * Natural command understanding as the Command Center sees it: the local
 * semantic model's status and download, the user's on/off choice, and the
 * last request's understanding diagnostics (Developer Mode).
 *
 * The model runs in SERSHI's own semantic engine and only interprets
 * requests; nothing here can run a tool or approve anything.
 */
import type {
  CommandOutcome,
  ModelError,
  SemanticStatus,
  UnderstandingTrace,
} from "@sershi/contracts";
import { create } from "zustand";

import { loadPreferences, savePreferences } from "../i18n/preferences";
import { desktopRuntime, sershi } from "../ipc";

/** The last request's understanding, for Developer Mode (memory only). */
export interface UnderstandingRecord {
  /** What was typed or heard, as shown in the conversation. */
  raw: string | null;
  trace: UnderstandingTrace;
  /** The outcome's status, e.g. "completed" or "needsClarification". */
  status: CommandOutcome["status"];
}

interface UnderstandingStore {
  status: SemanticStatus | null;
  enabled: boolean;
  error: ModelError | null;
  last: UnderstandingRecord | null;
  refresh: () => Promise<void>;
  setEnabled: (enabled: boolean) => void;
  download: () => void;
  cancelDownload: () => void;
  /** Records an outcome's diagnostics (called for every outcome). */
  observe: (outcome: CommandOutcome, raw: string | null) => void;
}

export const useUnderstanding = create<UnderstandingStore>((set, get) => ({
  status: null,
  enabled: loadPreferences().naturalUnderstanding,
  error: null,
  last: null,

  refresh: async () => {
    if (!desktopRuntime) return;
    try {
      set({ status: await sershi.getSemanticStatus() });
    } catch {
      // Unknown: the section says it is unavailable.
    }
  },

  setEnabled: (enabled) => {
    savePreferences({ ...loadPreferences(), naturalUnderstanding: enabled });
    set({ enabled });
    if (!desktopRuntime) return;
    sershi
      .configureSemantic({ enabled })
      .then((status) => {
        set({ status });
      })
      .catch(() => undefined);
  },

  download: () => {
    if (!desktopRuntime) return;
    set({ error: null });
    sershi
      .downloadSemanticModel()
      .then(() => get().refresh())
      .catch(() => {
        set({ error: "network" });
      });
  },

  cancelDownload: () => {
    if (desktopRuntime) sershi.cancelSemanticModelDownload().catch(() => undefined);
  },

  observe: (outcome, raw) => {
    if (outcome.understanding) {
      set({ last: { raw, trace: outcome.understanding, status: outcome.status } });
    }
  },
}));

/**
 * Connects to the core (Command Center only): pushes the stored choice and
 * follows downloads. Returns cleanup.
 */
export function connectUnderstanding(): () => void {
  if (!desktopRuntime) return () => undefined;
  const { enabled } = useUnderstanding.getState();
  sershi
    .configureSemantic({ enabled })
    .then((status) => {
      useUnderstanding.setState({ status });
    })
    .catch(() => undefined);
  return sershi.onSemanticModel((progress) => {
    const { status } = useUnderstanding.getState();
    if (status) {
      useUnderstanding.setState({
        status: { ...status, model: { ...status.model, state: progress.state } },
      });
    }
    if (progress.error) {
      useUnderstanding.setState({ error: progress.error });
    } else if (progress.state.kind === "installed") {
      useUnderstanding.setState({ error: null });
      void useUnderstanding.getState().refresh();
    }
  });
}
