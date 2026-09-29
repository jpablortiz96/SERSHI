/**
 * Interface preferences (language, motion, companion size), stored in the
 * WebView's localStorage.
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

export const MOTION_PREFERENCES: readonly MotionPreference[] = ["system", "reduced"];
export const COMPANION_SIZES: readonly CompanionSize[] = ["small", "medium", "large"];

export interface Preferences {
  uiLocale: LocalePreference;
  conversationLanguage: ConversationLanguage;
  motion: MotionPreference;
  companionSize: CompanionSize;
}

export const DEFAULT_PREFERENCES: Preferences = {
  uiLocale: "auto",
  conversationLanguage: "automatic",
  motion: "system",
  companionSize: "medium",
};

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
