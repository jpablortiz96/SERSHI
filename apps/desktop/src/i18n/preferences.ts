/**
 * Interface preferences (language, theme, motion, companion, sounds, global
 * shortcut), stored in the WebView's localStorage. Every value is
 * presentation or invocation only: nothing here is read by policy,
 * permissions or tools, so no preference can grant authority.
 *
 * Why not the core: SERSHI has no persistent settings store yet (SQLite
 * arrives in v0.1, ADR 0004), and these values are presentation-only — no
 * policy, permission or tool reads them. Both windows share one origin, so
 * they share this storage and are notified of each other's changes through
 * the `storage` event. Migration: the v0.1 settings store imports this key
 * once, then becomes the source of truth (docs/LOCALIZATION.md).
 */
import { isLocalePreference } from "./detect";
import type { ConversationLanguage, LocalePreference } from "./types";

export const PREFERENCES_KEY = "sershi.preferences.v1";

/** "system" follows the operating system's reduced-motion setting. */
export type MotionPreference = "system" | "reduced";
export type CompanionSize = "small" | "medium" | "large";

/** "system" follows the Windows light/dark app mode. */
export type ThemePreference = "system" | "light" | "dark";
/** Registered companion renderers (see visual/companions.ts). */
export type CompanionAppearance = "orbital";

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
};

/** Clamps a volume to 0–100; anything else becomes the default. */
export function clampVolume(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return DEFAULT_SOUND_VOLUME;
  return Math.min(100, Math.max(0, Math.round(value)));
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
