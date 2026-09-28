import { create } from "zustand";

import { detectSystemLocale, resolveLocale, systemLanguages } from "./detect";
import { loadPreferences, parsePreferences, PREFERENCES_KEY, savePreferences } from "./preferences";
import type { Locale, LocalePreference } from "./types";

interface LocaleStore {
  /** What the user chose ("auto" follows the system). */
  preference: LocalePreference;
  /** Locale detected from the operating system at start-up. */
  systemLocale: Locale;
  /** The locale actually rendered. */
  locale: Locale;
  setPreference: (preference: LocalePreference) => void;
}

function initial(): Pick<LocaleStore, "preference" | "systemLocale" | "locale"> {
  const preference = loadPreferences().uiLocale;
  const systemLocale = detectSystemLocale(systemLanguages());
  return { preference, systemLocale, locale: resolveLocale(preference, systemLocale) };
}

export const useLocaleStore = create<LocaleStore>((set, get) => ({
  ...initial(),
  setPreference: (preference) => {
    savePreferences({ ...loadPreferences(), uiLocale: preference });
    set({ preference, locale: resolveLocale(preference, get().systemLocale) });
  },
}));

/**
 * Keeps `<html lang>` in sync (screen-reader pronunciation) and follows
 * changes made in the other SERSHI window. Returns a cleanup function.
 */
export function connectLocale(): () => void {
  const apply = (locale: Locale) => {
    document.documentElement.lang = locale;
  };
  apply(useLocaleStore.getState().locale);
  const unsubscribe = useLocaleStore.subscribe((s) => {
    apply(s.locale);
  });

  const onStorage = (event: StorageEvent) => {
    if (event.key !== PREFERENCES_KEY) return;
    const preference = parsePreferences(event.newValue).uiLocale;
    const { systemLocale } = useLocaleStore.getState();
    useLocaleStore.setState({ preference, locale: resolveLocale(preference, systemLocale) });
  };
  window.addEventListener("storage", onStorage);
  return () => {
    unsubscribe();
    window.removeEventListener("storage", onStorage);
  };
}
