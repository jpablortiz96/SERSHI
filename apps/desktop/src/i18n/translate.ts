import { isLocale } from "./detect";
import { MESSAGES } from "./messages";
import { DEFAULT_LOCALE, type Locale, type MessageKey, type Translate } from "./types";

type Tree = { readonly [key: string]: string | Tree };

export function flattenMessages(tree: Tree, prefix = "", out = new Map<string, string>()) {
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") out.set(path, value);
    else flattenMessages(value, path, out);
  }
  return out;
}

const tables = new Map<Locale, Map<string, string>>();

function table(requested: Locale): Map<string, string> {
  // Defends against corrupted state: an unknown locale renders English.
  const locale = isLocale(requested) ? requested : DEFAULT_LOCALE;
  let t = tables.get(locale);
  if (!t) {
    t = flattenMessages(MESSAGES[locale]);
    tables.set(locale, t);
  }
  return t;
}

/** Replaces `{name}` placeholders. Unknown placeholders are left visible. */
export function interpolate(template: string, params?: Record<string, string | number>): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

/**
 * Creates a translator for `locale`. Missing entries fall back to English,
 * then to the key itself, so a gap is visible but never crashes the UI.
 */
export function createTranslator(locale: Locale): Translate {
  const primary = table(locale);
  const fallback = table("en-US");
  return (key: MessageKey, ...args: unknown[]) => {
    const template = primary.get(key) ?? fallback.get(key) ?? key;
    // `TranslateArgs` guarantees the first extra argument is the params record.
    return interpolate(template, args[0] as Record<string, string | number> | undefined);
  };
}
