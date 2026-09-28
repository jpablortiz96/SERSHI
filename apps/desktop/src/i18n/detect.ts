import { DEFAULT_LOCALE, SUPPORTED_LOCALES, type Locale, type LocalePreference } from "./types";

/**
 * Maps any BCP 47 tag to a supported locale by language:
 * `es-*` → es-419, `pt-*` → pt-BR, `en-*` → en-US. Returns null otherwise.
 */
export function matchLocale(tag: string | null | undefined): Locale | null {
  const language = tag?.trim().toLowerCase().split(/[-_]/)[0];
  switch (language) {
    case "es":
      return "es-419";
    case "pt":
      return "pt-BR";
    case "en":
      return "en-US";
    default:
      return null;
  }
}

/**
 * Picks the interface locale from the operating system's preferred languages
 * (most preferred first). The first language SERSHI supports wins; anything
 * else falls back to English.
 */
export function detectSystemLocale(languages: readonly string[]): Locale {
  for (const tag of languages) {
    const locale = matchLocale(tag);
    if (locale) return locale;
  }
  return DEFAULT_LOCALE;
}

/** The OS language list as exposed by the WebView (WebView2 follows the Windows display language). */
export function systemLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  return navigator.languages.length > 0 ? navigator.languages : [navigator.language];
}

export function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && (SUPPORTED_LOCALES as readonly string[]).includes(value);
}

export function isLocalePreference(value: unknown): value is LocalePreference {
  return value === "auto" || isLocale(value);
}

/** A manual choice always wins; "auto" follows the system. */
export function resolveLocale(preference: LocalePreference, systemLocale: Locale): Locale {
  return preference === "auto" ? systemLocale : preference;
}
