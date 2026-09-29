/**
 * Appearance preferences: motion and companion size. Presentation only —
 * nothing here can affect policy, permissions or tools. Stored with the
 * other interface preferences (i18n/preferences.ts) and shared by every
 * SERSHI window through the `storage` event.
 *
 * `<html data-motion>` is the single switch every stylesheet reads:
 * "reduced" when the user chose Reduced, or chose System and the operating
 * system asks for reduced motion.
 */
import { create } from "zustand";

import {
  loadPreferences,
  parsePreferences,
  PREFERENCES_KEY,
  savePreferences,
  type CompanionSize,
  type MotionPreference,
} from "../i18n/preferences";

export type { CompanionSize, MotionPreference } from "../i18n/preferences";
export { COMPANION_SIZES, MOTION_PREFERENCES } from "../i18n/preferences";

/** Visible core diameter of the companion, in CSS pixels. */
export const COMPANION_CORE_SIZE: Record<CompanionSize, number> = {
  small: 88,
  medium: 104,
  large: 120,
};

const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";

function systemPrefersReduced(): boolean {
  try {
    return typeof window !== "undefined" && window.matchMedia(REDUCED_QUERY).matches;
  } catch {
    return false;
  }
}

interface AppearanceStore {
  motion: MotionPreference;
  companionSize: CompanionSize;
  systemReduced: boolean;
  setMotion: (motion: MotionPreference) => void;
  setCompanionSize: (size: CompanionSize) => void;
}

export const useAppearance = create<AppearanceStore>((set) => {
  const stored = loadPreferences();
  return {
    motion: stored.motion,
    companionSize: stored.companionSize,
    systemReduced: systemPrefersReduced(),
    setMotion: (motion) => {
      savePreferences({ ...loadPreferences(), motion });
      set({ motion });
    },
    setCompanionSize: (companionSize) => {
      savePreferences({ ...loadPreferences(), companionSize });
      set({ companionSize });
    },
  };
});

/** Whether motion should be reduced right now. */
export function motionReduced(
  s: Pick<AppearanceStore, "motion" | "systemReduced"> = useAppearance.getState(),
): boolean {
  return s.motion === "reduced" || s.systemReduced;
}

/**
 * Applies appearance to `<html>` and follows changes from the operating
 * system and from other SERSHI windows. Returns a cleanup function.
 */
export function connectAppearance(): () => void {
  const html = document.documentElement;
  const apply = () => {
    const s = useAppearance.getState();
    html.dataset.motion = motionReduced(s) ? "reduced" : "full";
    html.dataset.companionSize = s.companionSize;
  };
  apply();
  const unsubscribe = useAppearance.subscribe(apply);

  let query: MediaQueryList | null = null;
  const onSystem = (e: MediaQueryListEvent) => {
    useAppearance.setState({ systemReduced: e.matches });
  };
  try {
    query = window.matchMedia(REDUCED_QUERY);
    query.addEventListener("change", onSystem);
  } catch {
    query = null;
  }

  const onStorage = (event: StorageEvent) => {
    if (event.key !== PREFERENCES_KEY) return;
    const { motion, companionSize } = parsePreferences(event.newValue);
    useAppearance.setState({ motion, companionSize });
  };
  window.addEventListener("storage", onStorage);

  return () => {
    unsubscribe();
    query?.removeEventListener("change", onSystem);
    window.removeEventListener("storage", onStorage);
  };
}
