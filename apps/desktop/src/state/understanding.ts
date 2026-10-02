/**
 * Natural understanding as the Command Center sees it: the two optional
 * local models — the Agent Brain (Prompt 4: conversation, follow-ups,
 * plans) and the semantic router (Gate 3C: short, imperfect commands) —
 * their status, downloads and on/off choices, plus the last request's
 * understanding diagnostics (Developer Mode).
 *
 * The models run in SERSHI's own local engine and only interpret and
 * propose; nothing here can run a tool or approve anything.
 */
import type {
  BrainTrace,
  CommandOutcome,
  ModelError,
  ModelProgress,
  SemanticStatus,
  UnderstandingTrace,
} from "@sershi/contracts";
import { create } from "zustand";

import { loadPreferences, savePreferences } from "../i18n/preferences";
import { desktopRuntime, sershi } from "../ipc";

export type ModelRole = "brain" | "semantic";
export const MODEL_ROLES: readonly ModelRole[] = ["brain", "semantic"];

/** The last request's understanding, for Developer Mode (memory only). */
export interface UnderstandingRecord {
  /** What was typed or heard, as shown in the conversation. */
  raw: string | null;
  trace: UnderstandingTrace | null;
  brain: BrainTrace | null;
  /** The outcome's status, e.g. "completed" or "needsClarification". */
  status: CommandOutcome["status"];
}

interface RoleState {
  status: SemanticStatus | null;
  enabled: boolean;
  error: ModelError | null;
}

interface UnderstandingStore {
  models: Record<ModelRole, RoleState>;
  last: UnderstandingRecord | null;
  refresh: (role: ModelRole) => Promise<void>;
  setEnabled: (role: ModelRole, enabled: boolean) => void;
  download: (role: ModelRole) => void;
  cancelDownload: (role: ModelRole) => void;
  /** Records an outcome's diagnostics (called for every outcome). */
  observe: (outcome: CommandOutcome, raw: string | null) => void;
}

const api = {
  brain: {
    status: () => sershi.getBrainStatus(),
    configure: (enabled: boolean) => sershi.configureBrain({ enabled }),
    download: () => sershi.downloadBrainModel(),
    cancel: () => sershi.cancelBrainModelDownload(),
    preference: "localBrain" as const,
  },
  semantic: {
    status: () => sershi.getSemanticStatus(),
    configure: (enabled: boolean) => sershi.configureSemantic({ enabled }),
    download: () => sershi.downloadSemanticModel(),
    cancel: () => sershi.cancelSemanticModelDownload(),
    preference: "naturalUnderstanding" as const,
  },
};

function initial(role: ModelRole): RoleState {
  return { status: null, enabled: loadPreferences()[api[role].preference], error: null };
}

export const useUnderstanding = create<UnderstandingStore>((set, get) => {
  const patch = (role: ModelRole, next: Partial<RoleState>) => {
    set((s) => ({ models: { ...s.models, [role]: { ...s.models[role], ...next } } }));
  };
  return {
    models: { brain: initial("brain"), semantic: initial("semantic") },
    last: null,

    refresh: async (role) => {
      if (!desktopRuntime) return;
      try {
        patch(role, { status: await api[role].status() });
      } catch {
        // Unknown: the card says it is unavailable.
      }
    },

    setEnabled: (role, enabled) => {
      savePreferences({ ...loadPreferences(), [api[role].preference]: enabled });
      patch(role, { enabled });
      if (!desktopRuntime) return;
      api[role]
        .configure(enabled)
        .then((status) => {
          patch(role, { status });
          // The other model's standby may change (one model at a time).
          void get().refresh(role === "brain" ? "semantic" : "brain");
        })
        .catch(() => undefined);
    },

    download: (role) => {
      if (!desktopRuntime) return;
      patch(role, { error: null });
      api[role]
        .download()
        .then(() => get().refresh(role))
        .catch(() => {
          // A refused start (e.g. not enough disk space) is reported by
          // the progress event; anything else is a network problem.
          if (!get().models[role].error) patch(role, { error: "network" });
        });
    },

    cancelDownload: (role) => {
      if (desktopRuntime) api[role].cancel().catch(() => undefined);
    },

    observe: (outcome, raw) => {
      if (outcome.understanding || outcome.brain) {
        set({
          last: {
            raw,
            trace: outcome.understanding,
            brain: outcome.brain,
            status: outcome.status,
          },
        });
      }
    },
  };
});

function follow(role: ModelRole) {
  return (progress: ModelProgress) => {
    const state = useUnderstanding.getState();
    const current = state.models[role];
    useUnderstanding.setState({
      models: {
        ...state.models,
        [role]: {
          ...current,
          status: current.status
            ? { ...current.status, model: { ...current.status.model, state: progress.state } }
            : current.status,
          error: progress.error ?? (progress.state.kind === "installed" ? null : current.error),
        },
      },
    });
    if (progress.state.kind === "installed") {
      for (const r of MODEL_ROLES) void useUnderstanding.getState().refresh(r);
    }
  };
}

/**
 * Connects to the core (Command Center only): pushes the stored choices and
 * follows downloads. Returns cleanup.
 */
export function connectUnderstanding(): () => void {
  if (!desktopRuntime) return () => undefined;
  const { models } = useUnderstanding.getState();
  for (const role of MODEL_ROLES) {
    api[role]
      .configure(models[role].enabled)
      .then((status) => {
        const s = useUnderstanding.getState();
        useUnderstanding.setState({
          models: { ...s.models, [role]: { ...s.models[role], status } },
        });
      })
      .catch(() => undefined);
  }
  const stopSemantic = sershi.onSemanticModel(follow("semantic"));
  const stopBrain = sershi.onBrainModel(follow("brain"));
  return () => {
    stopSemantic();
    stopBrain();
  };
}
