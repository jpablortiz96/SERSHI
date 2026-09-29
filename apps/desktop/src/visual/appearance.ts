/**
 * Appearance preferences: theme, motion, companion, interface sounds.
 * Presentation only — nothing here can affect policy, permissions or tools.
 * Stored with the other interface preferences (i18n/preferences.ts) and
 * shared by every SERSHI window through the `storage` event.
 *
 * `<html>` carries the switches every stylesheet reads:
 * - design tokens as custom properties (the resolved theme),
 * - `data-theme` / `color-scheme` (light or dark; native controls follow),
 * - `data-motion` ("reduced" from the user's choice or the OS).
 */
import { applyTheme } from "@sershi/design-tokens";
import { create } from "zustand";

import {
  clampVolume,
  loadPreferences,
  parsePreferences,
  PREFERENCES_KEY,
  savePreferences,
  type CompanionAppearance,
  type CompanionSize,
  type MotionPreference,
  type Preferences,
  type ThemePreference,
} from "../i18n/preferences";
import { resolveTheme, themeTokens, type ThemeId } from "./themes";

export type {
  CompanionAppearance,
  CompanionSize,
  MotionPreference,
  ThemePreference,
} from "../i18n/preferences";
export {
  COMPANION_APPEARANCES,
  COMPANION_SIZES,
  MOTION_PREFERENCES,
  THEME_PREFERENCES,
} from "../i18n/preferences";

/** Visible core diameter of the companion, in CSS pixels. */
export const COMPANION_CORE_SIZE: Record<CompanionSize, number> = {
  small: 88,
  medium: 104,
  large: 120,
};

const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";
const DARK_QUERY = "(prefers-color-scheme: dark)";

function queryMatches(query: string, fallback: boolean): boolean {
  try {
    return typeof window !== "undefined" ? window.matchMedia(query).matches : fallback;
  } catch {
    return fallback;
  }
}

type Stored = Pick<
  Preferences,
  "motion" | "companionSize" | "theme" | "companionAppearance" | "interfaceSounds" | "soundVolume"
>;

interface AppearanceStore extends Stored {
  systemReduced: boolean;
  /** The operating system's light/dark app mode (prefers-color-scheme). */
  systemDark: boolean;
  setMotion: (motion: MotionPreference) => void;
  setCompanionSize: (size: CompanionSize) => void;
  setTheme: (theme: ThemePreference) => void;
  setCompanionAppearance: (appearance: CompanionAppearance) => void;
  setInterfaceSounds: (enabled: boolean) => void;
  setSoundVolume: (volume: number) => void;
}

function stored(p: Preferences): Stored {
  return {
    motion: p.motion,
    companionSize: p.companionSize,
    theme: p.theme,
    companionAppearance: p.companionAppearance,
    interfaceSounds: p.interfaceSounds,
    soundVolume: p.soundVolume,
  };
}

export const useAppearance = create<AppearanceStore>((set) => {
  const save = (patch: Partial<Stored>) => {
    savePreferences({ ...loadPreferences(), ...patch });
    set(patch);
  };
  return {
    ...stored(loadPreferences()),
    systemReduced: queryMatches(REDUCED_QUERY, false),
    // Before the OS answers, assume dark: SERSHI's native window background
    // is dark, so this avoids a light flash.
    systemDark: queryMatches(DARK_QUERY, true),
    setMotion: (motion) => {
      save({ motion });
    },
    setCompanionSize: (companionSize) => {
      save({ companionSize });
    },
    setTheme: (theme) => {
      save({ theme });
    },
    setCompanionAppearance: (companionAppearance) => {
      save({ companionAppearance });
    },
    setInterfaceSounds: (interfaceSounds) => {
      save({ interfaceSounds });
    },
    setSoundVolume: (volume) => {
      save({ soundVolume: clampVolume(volume) });
    },
  };
});

/** Whether motion should be reduced right now. */
export function motionReduced(
  s: Pick<AppearanceStore, "motion" | "systemReduced"> = useAppearance.getState(),
): boolean {
  return s.motion === "reduced" || s.systemReduced;
}

/** The theme shown right now. */
export function activeTheme(
  s: Pick<AppearanceStore, "theme" | "systemDark"> = useAppearance.getState(),
): ThemeId {
  return resolveTheme(s.theme, s.systemDark);
}

/**
 * Applies appearance to `<html>` and follows changes from the operating
 * system and from other SERSHI windows. Tokens are rewritten only when the
 * resolved theme changes — never by re-rendering the application. Returns a
 * cleanup function.
 */
export function connectAppearance(): () => void {
  const html = document.documentElement;
  let applied: ThemeId | null = null;
  const apply = () => {
    const s = useAppearance.getState();
    const theme = activeTheme(s);
    if (theme !== applied) {
      applyTheme(html, themeTokens(theme));
      html.dataset.theme = theme;
      html.style.colorScheme = theme;
      applied = theme;
    }
    html.dataset.motion = motionReduced(s) ? "reduced" : "full";
    html.dataset.companionSize = s.companionSize;
  };
  apply();
  const unsubscribe = useAppearance.subscribe(apply);

  const listeners: [MediaQueryList, (e: MediaQueryListEvent) => void][] = [];
  const watch = (query: string, onChange: (matches: boolean) => void) => {
    try {
      const list = window.matchMedia(query);
      const handler = (e: MediaQueryListEvent) => {
        onChange(e.matches);
      };
      list.addEventListener("change", handler);
      listeners.push([list, handler]);
    } catch {
      // No media queries (very old runtime): the preference still applies.
    }
  };
  watch(REDUCED_QUERY, (systemReduced) => {
    useAppearance.setState({ systemReduced });
  });
  watch(DARK_QUERY, (systemDark) => {
    useAppearance.setState({ systemDark });
  });

  const onStorage = (event: StorageEvent) => {
    if (event.key !== PREFERENCES_KEY) return;
    useAppearance.setState(stored(parsePreferences(event.newValue)));
  };
  window.addEventListener("storage", onStorage);

  return () => {
    unsubscribe();
    for (const [list, handler] of listeners) list.removeEventListener("change", handler);
    window.removeEventListener("storage", onStorage);
  };
}
