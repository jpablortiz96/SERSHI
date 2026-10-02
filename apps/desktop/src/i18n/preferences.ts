/**
 * Interface preferences (language, theme, motion, companion, sounds, global
 * shortcut, voice), stored in the WebView's localStorage. Every value is
 * presentation or invocation only: nothing here is read by policy,
 * permissions or tools, so no preference can grant authority. Voice
 * preferences choose devices and languages; they cannot approve anything.
 *
 * Why not the core: SERSHI has no persistent settings store yet (SQLite
 * arrives in v0.1, ADR 0004), and these values are presentation-only — no
 * policy, permission or tool reads them. Both windows share one origin, so
 * they share this storage and are notified of each other's changes through
 * the `storage` event. Migration: the v0.1 settings store imports this key
 * once, then becomes the source of truth (docs/LOCALIZATION.md).
 */
import { isLocalePreference } from "./detect";
import { CONVERSATION_LANGUAGES, type ConversationLanguage, type LocalePreference } from "./types";

export const PREFERENCES_KEY = "sershi.preferences.v1";

/** "system" follows the operating system's reduced-motion setting. */
export type MotionPreference = "system" | "reduced";
export type CompanionSize = "small" | "medium" | "large";

/** "system" follows the Windows light/dark app mode. */
export type ThemePreference = "system" | "light" | "dark";
/** Registered companion renderers (see visual/companions.ts). */
export type CompanionAppearance = "orbital";

/** Speech recognition profiles (docs/VOICE.md#models). */
export type SpeechProfile = "fast" | "accurate";
export const SPEECH_PROFILES: readonly SpeechProfile[] = ["fast", "accurate"];
/** End-of-speech overrides offered in Developer Mode (ms). */
export const ENDPOINT_OVERRIDES: readonly number[] = [450, 550, 650, 750];

export const MOTION_PREFERENCES: readonly MotionPreference[] = ["system", "reduced"];
export const COMPANION_SIZES: readonly CompanionSize[] = ["small", "medium", "large"];
export const THEME_PREFERENCES: readonly ThemePreference[] = ["system", "light", "dark"];
export const COMPANION_APPEARANCES: readonly CompanionAppearance[] = ["orbital"];

/** Default interface-sound volume (0–100): deliberately quiet. */
export const DEFAULT_SOUND_VOLUME = 35;

export interface Preferences {
  uiLocale: LocalePreference;
  conversationLanguage: ConversationLanguage;
  motion: MotionPreference;
  companionSize: CompanionSize;
  theme: ThemePreference;
  companionAppearance: CompanionAppearance;
  /** Non-voice interface sounds. Off until the user turns them on. */
  interfaceSounds: boolean;
  /** 0–100. */
  soundVolume: number;
  /**
   * The global shortcut the user chose, or null for SERSHI's default.
   * Invocation only — it never grants any capability.
   */
  shortcut: string | null;
  /** Speak replies to voice requests. */
  voiceResponses: boolean;
  /** Also speak replies to typed requests. Off: typing stays silent. */
  speakTypedResponses: boolean;
  /** Microphone endpoint id, or null for the system default. */
  microphone: string | null;
  /** Speech voice id, or null to pick one for the reply's language. */
  speechVoice: string | null;
  /** Speech recognition profile ("fast" is the default). */
  speechProfile: SpeechProfile;
  /**
   * Developer tuning only (Developer Mode): a fixed end-of-speech silence in
   * ms instead of the adaptive default; null = adaptive.
   */
  endpointMs: number | null;
  /**
   * Natural command understanding with the local semantic model, once it
   * is installed (on by default). The model only interprets requests.
   */
  naturalUnderstanding: boolean;
  /**
   * The local Agent Brain (Prompt 4), once installed (on by default). It
   * proposes; SERSHI's rules decide.
   */
  localBrain: boolean;
}

export const DEFAULT_PREFERENCES: Preferences = {
  uiLocale: "auto",
  conversationLanguage: "automatic",
  motion: "system",
  companionSize: "medium",
  theme: "system",
  companionAppearance: "orbital",
  interfaceSounds: false,
  soundVolume: DEFAULT_SOUND_VOLUME,
  shortcut: null,
  voiceResponses: true,
  speakTypedResponses: false,
  microphone: null,
  speechVoice: null,
  speechProfile: "fast",
  endpointMs: null,
  naturalUnderstanding: true,
  localBrain: true,
};

/** Clamps a volume to 0–100; anything else becomes the default. */
export function clampVolume(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return DEFAULT_SOUND_VOLUME;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function printable(value: string): boolean {
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i);
    if (code < 32 || code === 127) return false;
  }
  return true;
}

/** Device, voice and model ids: short printable text, or null. */
function parseId(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 && value.length <= 512 && printable(value)
    ? value
    : null;
}

/** Accelerator strings are short, printable and made of +-joined tokens. */
function parseShortcut(value: unknown): string | null {
  return typeof value === "string" && /^[A-Za-z0-9+]{3,40}$/.test(value) ? value : null;
}

const oneOf = <T extends string>(options: readonly T[], value: unknown, fallback: T): T =>
  typeof value === "string" && (options as readonly string[]).includes(value)
    ? (value as T)
    : fallback;

/** Parses stored preferences; anything unrecognised falls back to defaults. */
export function parsePreferences(raw: string | null): Preferences {
  if (!raw) return DEFAULT_PREFERENCES;
  try {
    const value: unknown = JSON.parse(raw);
    if (typeof value !== "object" || value === null) return DEFAULT_PREFERENCES;
    const uiLocale = "uiLocale" in value ? value.uiLocale : undefined;
    return {
      ...DEFAULT_PREFERENCES,
      uiLocale: isLocalePreference(uiLocale) ? uiLocale : DEFAULT_PREFERENCES.uiLocale,
      motion: oneOf(
        MOTION_PREFERENCES,
        "motion" in value ? value.motion : undefined,
        DEFAULT_PREFERENCES.motion,
      ),
      companionSize: oneOf(
        COMPANION_SIZES,
        "companionSize" in value ? value.companionSize : undefined,
        DEFAULT_PREFERENCES.companionSize,
      ),
      theme: oneOf(
        THEME_PREFERENCES,
        "theme" in value ? value.theme : undefined,
        DEFAULT_PREFERENCES.theme,
      ),
      // An unknown renderer (e.g. from a newer version) falls back to Orbital.
      companionAppearance: oneOf(
        COMPANION_APPEARANCES,
        "companionAppearance" in value ? value.companionAppearance : undefined,
        DEFAULT_PREFERENCES.companionAppearance,
      ),
      interfaceSounds: "interfaceSounds" in value && value.interfaceSounds === true,
      soundVolume: clampVolume("soundVolume" in value ? value.soundVolume : undefined),
      shortcut: parseShortcut("shortcut" in value ? value.shortcut : undefined),
      conversationLanguage: oneOf(
        CONVERSATION_LANGUAGES,
        "conversationLanguage" in value ? value.conversationLanguage : undefined,
        DEFAULT_PREFERENCES.conversationLanguage,
      ),
      voiceResponses: !("voiceResponses" in value && value.voiceResponses === false),
      speakTypedResponses: "speakTypedResponses" in value && value.speakTypedResponses === true,
      microphone: parseId("microphone" in value ? value.microphone : undefined),
      speechVoice: parseId("speechVoice" in value ? value.speechVoice : undefined),
      speechProfile: oneOf(
        SPEECH_PROFILES,
        "speechProfile" in value ? value.speechProfile : undefined,
        DEFAULT_PREFERENCES.speechProfile,
      ),
      endpointMs:
        "endpointMs" in value &&
        typeof value.endpointMs === "number" &&
        ENDPOINT_OVERRIDES.includes(value.endpointMs)
          ? value.endpointMs
          : null,
      naturalUnderstanding: !(
        "naturalUnderstanding" in value && value.naturalUnderstanding === false
      ),
      localBrain: !("localBrain" in value && value.localBrain === false),
    };
  } catch {
    return DEFAULT_PREFERENCES;
  }
}

function storage(): Storage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

export function loadPreferences(): Preferences {
  try {
    return parsePreferences(storage()?.getItem(PREFERENCES_KEY) ?? null);
  } catch {
    return DEFAULT_PREFERENCES;
  }
}

export function savePreferences(preferences: Preferences): void {
  try {
    storage()?.setItem(PREFERENCES_KEY, JSON.stringify(preferences));
  } catch {
    // Storage full or unavailable: the choice still applies for this session.
  }
}
