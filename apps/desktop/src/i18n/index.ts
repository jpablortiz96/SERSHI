import { useMemo } from "react";

import { createFormatters, type Formatters } from "./format";
import { useLocaleStore } from "./store";
import { createTranslator } from "./translate";
import type { Locale, Translate } from "./types";

export { connectLocale, useLocaleStore } from "./store";
export { MESSAGES } from "./messages";
export { SUPPORTED_LOCALES, type Locale, type LocalePreference, type MessageKey } from "./types";
export type { Formatters } from "./format";
export type { Translate } from "./types";

export interface I18n {
  locale: Locale;
  t: Translate;
  format: Formatters;
}

/** The active locale's translator and formatters; re-renders on language change. */
export function useI18n(): I18n {
  const locale = useLocaleStore((s) => s.locale);
  return useMemo(
    () => ({ locale, t: createTranslator(locale), format: createFormatters(locale) }),
    [locale],
  );
}
