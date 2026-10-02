/**
 * Voice as the Command Center sees it: push-to-talk, what SERSHI heard,
 * spoken replies, microphone/voice/model preferences and model downloads.
 *
 * The microphone, recognition and speech run natively in SERSHI's core
 * process. This store never receives audio (only a bounded level, handled
 * in visual/voiceLevel.ts), never submits a transcript itself (the core
 * already did, through the typed-command path) and has no way to approve
 * anything.
 */
import type {
  CaptureStart,
  ModelError,
  VoiceFailure,
  VoiceSessionStatus,
  VoiceSettings,
  VoiceStatus,
  VoiceTimings,
  VoiceUpdate,
} from "@sershi/contracts";
import { create } from "zustand";

import { loadPreferences, savePreferences, type Preferences } from "../i18n/preferences";
import { useLocaleStore } from "../i18n/store";
import type { ConversationLanguage } from "../i18n/types";
import { desktopRuntime, sershi } from "../ipc";
import { useConversation } from "./conversation";
import { useUnderstanding } from "./understanding";
import {
  clearSpeechExpectation,
  expectSpeech,
  speakMessage,
  speakOutcome,
  stopSpeaking as stopSpeech,
} from "./speech";

export type VoicePrefs = Pick<
  Preferences,
  | "conversationLanguage"
  | "voiceResponses"
  | "speakTypedResponses"
  | "microphone"
  | "speechVoice"
  | "speechProfile"
  | "endpointMs"
>;

/** The last spoken command's latency record (diagnostics only). */
export interface LatencyRecord {
  timings: VoiceTimings;
  /** Outcome → spoken reply start, when a reply was spoken. */
  speechMs: number | null;
}

/** Something the command bar should explain (cleared by the next attempt). */
export type VoiceNotice =
  | { kind: "failure"; reason: VoiceFailure }
  | { kind: "fallback" }
  /** Push-to-talk needs a model first: offer the download. */
  | { kind: "modelRequired" }
  | { kind: "modelError"; error: ModelError }
  | { kind: "modelReady" };

interface VoiceStore {
  status: VoiceStatus | null;
  prefs: VoicePrefs;
  notice: VoiceNotice | null;
  /** Where the time went for the last spoken command (Developer Mode). */
  latency: LatencyRecord | null;
  /**
   * The hands-free voice session while one is active (Gate 4.1). The
   * microphone listens again after each reply only inside a session.
   */
  session: VoiceSessionStatus | null;
  /** The last session's interruptions, for diagnostics. */
  lastSession: VoiceSessionStatus | null;
  /** Speech was heard while the speech model is still loading (cold start). */
  preparing: boolean;
  startSession: () => Promise<void>;
  stopSession: () => void;
  refresh: () => Promise<void>;
  setPrefs: (patch: Partial<VoicePrefs>) => void;
  /** Push-to-talk: start, or stop and send if already listening. */
  toggleCapture: (listening: boolean) => Promise<void>;
  cancelCapture: () => void;
  stopSpeaking: () => void;
  download: (model: string) => void;
  cancelDownload: () => void;
  dismissNotice: () => void;
}

function prefsFrom(p: Preferences): VoicePrefs {
  return {
    conversationLanguage: p.conversationLanguage,
    voiceResponses: p.voiceResponses,
    speakTypedResponses: p.speakTypedResponses,
    microphone: p.microphone,
    speechVoice: p.speechVoice,
    speechProfile: p.speechProfile,
    endpointMs: p.endpointMs,
  };
}

/**
 * What the core needs from the preferences. The interface language is only
 * a hint for Automatic recognition's language retries (Gate 3C.1); replies
 * are always phrased and spoken in the interface language regardless of
 * what language recognition detected.
 */
export function voiceSettings(
  prefs: VoicePrefs,
  interfaceLanguage: string | null = useLocaleStore.getState().locale,
): VoiceSettings {
  const language: ConversationLanguage = prefs.conversationLanguage;
  return {
    interfaceLanguage,
    microphone: prefs.microphone,
    language: language === "automatic" ? null : language,
    voice: prefs.speechVoice,
    profile: prefs.speechProfile,
    endpointMs: prefs.endpointMs,
  };
}

export const useVoice = create<VoiceStore>((set, get) => ({
  status: null,
  prefs: prefsFrom(loadPreferences()),
  notice: null,
  latency: null,
  session: null,
  lastSession: null,
  preparing: false,

  startSession: async () => {
    if (!desktopRuntime) return;
    set({ notice: null });
    clearSpeechExpectation();
    let result: CaptureStart;
    try {
      result = await sershi.startVoiceSession();
    } catch {
      set({ notice: { kind: "failure", reason: "recognitionFailed" } });
      return;
    }
    if (result.kind === "refused") {
      set({
        notice:
          result.reason === "modelMissing"
            ? { kind: "modelRequired" }
            : { kind: "failure", reason: result.reason },
      });
    }
  },

  stopSession: () => {
    stopSpeech();
    if (desktopRuntime) sershi.stopVoiceSession().catch(() => undefined);
  },

  refresh: async () => {
    if (!desktopRuntime) return;
    try {
      set({ status: await sershi.getVoiceStatus() });
    } catch {
      // Status stays unknown; the microphone button stays disabled.
    }
  },

  setPrefs: (patch) => {
    const prefs = { ...get().prefs, ...patch };
    savePreferences({ ...loadPreferences(), ...prefs });
    set({ prefs });
    if (!desktopRuntime) return;
    sershi
      .configureVoice(voiceSettings(prefs))
      .then((status) => {
        set({ status });
      })
      .catch(() => undefined);
  },

  toggleCapture: async (listening) => {
    if (!desktopRuntime) return;
    if (listening) {
      await sershi.stopVoiceCapture().catch(() => undefined);
      return;
    }
    set({ notice: null });
    clearSpeechExpectation();
    let result: CaptureStart;
    try {
      result = await sershi.startVoiceCapture();
    } catch {
      set({ notice: { kind: "failure", reason: "recognitionFailed" } });
      return;
    }
    if (result.kind === "refused") {
      set({
        notice:
          result.reason === "modelMissing"
            ? { kind: "modelRequired" }
            : { kind: "failure", reason: result.reason },
      });
    }
  },

  cancelCapture: () => {
    if (desktopRuntime) sershi.cancelVoiceCapture().catch(() => undefined);
  },

  stopSpeaking: () => {
    stopSpeech();
  },

  download: (model) => {
    if (!desktopRuntime) return;
    set({ notice: null });
    sershi
      .downloadVoiceModel(model)
      .then(() => get().refresh())
      .catch(() => {
        set({ notice: { kind: "modelError", error: "network" } });
      });
  },

  cancelDownload: () => {
    if (desktopRuntime) sershi.cancelVoiceModelDownload().catch(() => undefined);
  },

  dismissNotice: () => {
    set({ notice: null });
  },
}));

/** Handles one voice event from the core. Exported for tests. */
export function handleVoiceUpdate(update: VoiceUpdate): void {
  const { prefs } = useVoice.getState();
  const conversation = useConversation.getState();
  switch (update.kind) {
    case "heard":
      useVoice.setState({ preparing: false });
      // "You said …" — shown before the command runs.
      conversation.addHeard(update.text, update.firstHeard);
      if (prefs.voiceResponses) expectSpeech();
      return;
    case "answered": {
      conversation.addReply({ kind: "outcome", outcome: update.outcome });
      if (update.anythingElse) conversation.addReply({ kind: "session", notice: "anythingElse" });
      const heard = [...useConversation.getState().messages]
        .reverse()
        .find((m) => m.role === "user" && m.via === "voice");
      useUnderstanding
        .getState()
        .observe(update.outcome, heard?.role === "user" ? heard.text : null);
      // A reply the user already talked over is shown, never spoken.
      if (prefs.voiceResponses && update.speak) speakOutcome(update.outcome, update.anythingElse);
      else if (!update.speak) clearSpeechExpectation();
      return;
    }
    case "preparing":
      useVoice.setState({ preparing: true });
      return;
    case "prompt":
      // "Sí" to "anything else?": SERSHI is listening (nothing ran).
      conversation.addReply({ kind: "session", notice: "listening" });
      if (prefs.voiceResponses) speakMessage("session.listening");
      return;
    case "noSpeech":
    case "unclear":
      useVoice.setState({ preparing: false });
      conversation.addReply({ kind: "voice", notice: update.kind });
      if (prefs.voiceResponses) speakMessage(`voice.${update.kind}`);
      return;
    case "failed":
      clearSpeechExpectation();
      useVoice.setState({ preparing: false });
      useVoice.setState({
        notice:
          update.reason === "modelMissing"
            ? { kind: "modelRequired" }
            : { kind: "failure", reason: update.reason },
      });
      return;
    case "deviceFallback":
      useVoice.setState({ notice: { kind: "fallback" } });
      return;
    case "cancelled":
      clearSpeechExpectation();
      useVoice.setState({ preparing: false });
      return;
    case "timings":
      useVoice.setState({ latency: { timings: update.timings, speechMs: null } });
      return;
    case "speechLatency": {
      const { latency } = useVoice.getState();
      if (latency) useVoice.setState({ latency: { ...latency, speechMs: update.ms } });
      return;
    }
  }
}

/** Follows the voice session's status (Gate 4.1). Exported for tests. */
export function handleVoiceSession(status: VoiceSessionStatus): void {
  const { prefs, session } = useVoice.getState();
  if (status.phase !== null) {
    const starting = session === null || session.id !== status.id;
    useVoice.setState({ session: status });
    if (starting) useConversation.getState().addReply({ kind: "session", notice: "listening" });
    return;
  }
  // Ended: the microphone is closed; say why (a farewell is answered).
  useVoice.setState({ session: null, lastSession: status });
  const reason = status.ended ?? "stopped";
  useConversation.getState().addReply({ kind: "session", notice: reason });
  if (prefs.voiceResponses && reason === "farewell") speakMessage("session.ended.farewell");
  if (prefs.voiceResponses && reason === "notHeard") speakMessage("session.ended.notHeard");
}

/**
 * Connects voice to the core (Command Center only, so events are handled
 * once). Pushes the stored preferences, follows downloads. Returns cleanup.
 */
export function connectVoice(): () => void {
  if (!desktopRuntime) return () => undefined;
  const store = useVoice.getState();
  sershi
    .configureVoice(voiceSettings(store.prefs))
    .then((status) => {
      useVoice.setState({ status });
    })
    .catch(() => undefined);
  const stopVoice = sershi.onVoice(handleVoiceUpdate);
  const stopSession = sershi.onVoiceSession(handleVoiceSession);
  // A session may already be running (the Command Center was reloaded).
  sershi
    .getVoiceSession()
    .then((status) => {
      useVoice.setState({ session: status?.phase ? status : null });
    })
    .catch(() => undefined);
  // The interface language is a recognition hint: keep the core in sync.
  const stopLocale = useLocaleStore.subscribe((state, previous) => {
    if (state.locale === previous.locale) return;
    sershi
      .configureVoice(voiceSettings(useVoice.getState().prefs, state.locale))
      .then((status) => {
        useVoice.setState({ status });
      })
      .catch(() => undefined);
  });
  const stopModel = sershi.onVoiceModel((progress) => {
    const { status } = useVoice.getState();
    if (status) {
      useVoice.setState({
        status: {
          ...status,
          models: status.models.map((m) =>
            m.id === progress.model ? { ...m, state: progress.state } : m,
          ),
        },
      });
    }
    if (progress.error) {
      useVoice.setState({ notice: { kind: "modelError", error: progress.error } });
    } else if (progress.state.kind === "installed") {
      useVoice.setState({ notice: { kind: "modelReady" } });
      void useVoice.getState().refresh();
    }
  });
  return () => {
    stopVoice();
    stopSession();
    stopModel();
    stopLocale();
  };
}
