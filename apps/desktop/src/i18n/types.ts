import type { enUS } from "./locales/en-US";

export const SUPPORTED_LOCALES = ["en-US", "es-419", "pt-BR"] as const;

/** A locale SERSHI's interface is translated into. */
export type Locale = (typeof SUPPORTED_LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "en-US";

/** What the user chose: follow the operating system, or a specific locale. */
export type LocalePreference = "auto" | Locale;

/**
 * The language SERSHI *understands and answers in* during conversation.
 * Deliberately separate from the interface locale: a user may keep the UI in
 * English and speak Spanish. Only `automatic` exists until voice and language
 * models arrive (docs/LOCALIZATION.md).
 */
export type ConversationLanguage = "automatic";

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
