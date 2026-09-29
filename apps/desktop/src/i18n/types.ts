import type { enUS } from "./locales/en-US";

export const SUPPORTED_LOCALES = ["en-US", "es-419", "pt-BR"] as const;

/** A locale SERSHI's interface is translated into. */
export type Locale = (typeof SUPPORTED_LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "en-US";

/** What the user chose: follow the operating system, or a specific locale. */
export type LocalePreference = "auto" | Locale;

/**
 * The language SERSHI *hears* in conversation (speech recognition).
 * Deliberately separate from the interface locale: a user may keep the UI in
 * Spanish and speak English. `automatic` lets the recogniser detect it;
 * otherwise a BCP-47 language subtag (docs/LOCALIZATION.md, docs/VOICE.md).
 */
export type ConversationLanguage = "automatic" | "en" | "es" | "pt";

export const CONVERSATION_LANGUAGES: readonly ConversationLanguage[] = [
  "automatic",
  "en",
  "es",
  "pt",
];

type Widen<T> = { [K in keyof T]: T[K] extends string ? string : Widen<T[K]> };

/** The shape every locale must match exactly (derived from en-US). */
export type Messages = Widen<typeof enUS>;

type Paths<T, Prefix extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? `${Prefix}${K}` : Paths<T[K], `${Prefix}${K}.`>;
}[keyof T & string];

/** Every valid translation key, e.g. `"nav.home"` or `"state.idle.label"`. */
export type MessageKey = Paths<Messages>;

type ValueAt<T, K extends string> = K extends `${infer Head}.${infer Rest}`
  ? Head extends keyof T
    ? ValueAt<T[Head], Rest>
    : never
  : K extends keyof T
    ? T[K]
    : never;

type Placeholders<S> = S extends `${string}{${infer Name}}${infer Rest}`
  ? Name | Placeholders<Rest>
  : never;

/** Placeholder names of a key, read from the canonical English message. */
export type ParamsOf<K extends MessageKey> = Placeholders<ValueAt<typeof enUS, K>>;

export type TranslateArgs<K extends MessageKey> = [ParamsOf<K>] extends [never]
  ? []
  : [params: Record<ParamsOf<K>, string | number>];

export type Translate = <K extends MessageKey>(key: K, ...args: TranslateArgs<K>) => string;

/** Keys whose message has no placeholders. */
export type PlainKey = {
  [K in MessageKey]: [ParamsOf<K>] extends [never] ? K : never;
}[MessageKey];
