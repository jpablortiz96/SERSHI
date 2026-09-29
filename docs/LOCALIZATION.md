# Localization

SERSHI is multilingual from the foundation. Every user-facing string in the
interface lives in a locale resource. Nothing about language affects
permissions, risk, confirmation or tool policy.

## Supported locales

| Locale   | Language                           | Notes                                               |
| -------- | ---------------------------------- | --------------------------------------------------- |
| `en-US`  | English                            | Canonical schema; fallback for everything           |
| `es-419` | Español (Latinoamérica)            | Neutral Latin American Spanish, informal *tú*, "computadora" |
| `pt-BR`  | Português (Brasil)                 | Brazilian Portuguese, *você*, "computador"          |

## Two separate concepts

| Concept                    | Meaning                                              | Today                         |
| -------------------------- | ---------------------------------------------------- | ----------------------------- |
| **`uiLocale`**             | The language SERSHI's interface is displayed in      | Automatic or a manual choice  |
| **`conversationLanguage`** | The language SERSHI understands and answers in       | `automatic` (the only value)  |

They are independent on purpose: someone may keep the interface in English and
speak Spanish. When voice (v0.3) and language models (v0.1) arrive, the
conversation language will be detected per request, or set by the user, without
touching the interface language. Model-generated replies will follow the
conversation language; fixed product copy follows the interface language.

The rule-based intent resolver already recognises English, Spanish and
Portuguese keywords (`crates/sershi-core/src/intent.rs`). This is vocabulary
matching so the suggestions shown in every interface language work. It is not
language detection. Application commands work in all three languages whatever
the interface language: open (`open`/`launch`/`start`/`run`,
`abre`/`abrir`/`inicia`/`ejecuta`, `abra`/`abrir`/`inicie`/`execute`) and close
(`close`/`quit`/`exit`, `cierra`/`cerrar`, `feche`/`fechar`/`encerre`). Only
imperatives act; see [APPLICATIONS.md](APPLICATIONS.md#intent-parsing).

## Locale selection

1. **Manual choice wins.** Settings → General → Language: *Automatic — System
   language*, *English*, *Español*, *Português*. A manual choice is remembered and
   never re-inferred.
2. **Automatic** follows the operating system: the WebView's preferred language
   list (`navigator.languages`; on Windows, WebView2 reports the Windows display
   language). The first supported language wins:

   | System language | Interface |
   | --------------- | --------- |
   | `es-*`          | `es-419`  |
   | `pt-*`          | `pt-BR`   |
   | `en-*`          | `en-US`   |
   | anything else   | `en-US`   |

3. Changes apply immediately in both windows. There is no restart. `<html lang>`
   is updated so screen readers pronounce text correctly.

REQUIRES_WINDOWS_VALIDATION: that WebView2 exposes the Windows display language
through `navigator.languages`. It was validated in Chromium (the WebView2 engine)
with a `pt-BR` browser locale. If it proves unreliable, add the OS locale to
`RuntimeInfo` from Rust (e.g. `GetUserDefaultLocaleName`) and pass it to
`detectSystemLocale`.

## Persistence

Stored in the WebView's `localStorage` under `sershi.preferences.v1`:

```json
{ "uiLocale": "auto", "conversationLanguage": "automatic" }
```

This is a deliberate temporary mechanism. SERSHI has no settings store yet
(SQLite arrives in v0.1, [ADR 0004](adr/0004-persistence.md)), and these values
are presentation-only: no policy, permission or tool reads them. Both windows
share the origin, so they share the value and follow each other's changes
through the `storage` event. Corrupt or unknown values fall back to defaults.

**Migration (v0.1):** the settings store becomes the source of truth. On first
launch it imports `sershi.preferences.v1`, then the UI reads and writes the
preference through a typed IPC command. The key can be removed after one
release.

## Architecture

```text
apps/desktop/src/i18n/
├── locales/en-US.ts   canonical messages (`as const`) — defines the schema
├── locales/es-419.ts  typed as `Messages`
├── locales/pt-BR.ts   typed as `Messages`
├── types.ts           Locale, Messages, MessageKey, placeholder types
├── messages.ts        locale → messages registry
├── translate.ts       createTranslator(): lookup, English fallback, {param} interpolation
├── format.ts          Intl-based number, percent, time, duration formatters
├── detect.ts          OS language → locale mapping, preference resolution
├── preferences.ts     load/save/parse stored preferences
├── store.ts           Zustand store; <html lang>; cross-window sync
├── domain.ts          core ids → copy (tools, capabilities, states, activity, outcomes)
└── index.ts           useI18n() → { locale, t, format }
```

There is no i18n framework dependency. The needs are typed keys, `{param}`
interpolation and `Intl` formatting, which fit in ~300 lines. A framework
(i18next, FormatJS) becomes worthwhile when SERSHI needs ICU plural/select
messages or translator tooling. Plurals are avoided in copy until then.

### Type safety

- `MessageKey` is the union of every dotted path in `en-US`. `t("nav.hom")` does
  not compile.
- Placeholder names are read from the English template literal:
  `t("reply.memory")` without `{ used, total, percent }` does not compile.
- Other locales are typed as `Messages`, so a missing or extra key is a compile
  error.
- Tests additionally check that every locale has the same keys, no empty
  messages, the same placeholders per key, labels for every assistant state, and
  that translations are not accidental copies of English.

### Core output is data, not sentences

The Rust core returns structured facts (`CommandOutcome.status`, `toolId`,
`data`, `detail`; `ActivityEntry.kind`, `toolId`), and the UI phrases them in
the interface language (`i18n/domain.ts`). The core's English `reply` and
`summary` remain as the canonical fallback: they are shown if the UI meets
something it cannot phrase, such as a new tool without translations yet. The
conversation stores outcomes rather than rendered text, so past replies
re-render when the language changes.

### Application names

Built-in Windows applications have localized display names in the UI
(`apps.calculator`, `apps.notepad`, `apps.explorer`, `apps.settings`) and are
recognised under their names in every supported language (“Calculadora”,
“Bloc de notas”, “Explorador de archivos”, “Configuración”, “Configurações”…).
Third-party application names come from the system as discovered and are never
translated.

### Surfaces outside the WebView

- **Tray menu.** Rust creates it with English labels; the Command Center sends
  the current language's `tray.*` labels through `set_tray_labels` at start-up
  and whenever the language changes. Rust validates them (≤ 48 characters, no
  control characters); the actions behind each item are fixed and cannot be
  changed by the labels.
- **Confirmation window.** A separate window (`confirmation.html`) that
  renders structured fields (`action`, `level`, `subject`, `reason`) with the
  `confirm.*` messages. It reuses the interface-language preference the other
  windows share (same origin); the language is presentation only and never
  part of authorization. It never shows free text from the request, a tool or
  a model. Built-in application names are localized by id.

### Appearance and visual copy

The following are localized like every other string:

- Settings → Appearance: theme, motion, companion, companion size,
  interface sounds and volume
- Settings → Windows integration → Global shortcut: the recorder prompt,
  rejection reasons, "Shortcut unavailable" and the AltGr warning
- activity status words
- the confirmation window's approval mark ("SERSHI approval" /
  "Aprobación de SERSHI" / "Aprovação do SERSHI")

Some names are product names and stay untranslated: the theme names
"SERSHI Dark" and "SERSHI Light" and the companion name "Orbital".

Other rules:

- Key names in shortcuts (`Ctrl`, `Alt`, `Shift`) follow Windows'
  keyboard labels in every language.
- The volume value follows each locale's percent convention (`35%`,
  `35 %`).
- Navigation measures its tabs, so longer Spanish and Portuguese labels
  never clip.

### What is not localized

Tool ids (`system.get_memory`), permission ids, capability ids, Rust enum values,
milestone names (`v0.1`), log and diagnostic text, the OS and CPU names reported
by the system, and the brand name SERSHI.

### Formatting

Numbers, percentages, times and durations use `Intl` with the active locale,
following CLDR conventions. For example, `pt-BR` renders `1,5 GB`, `67%` and
`15:59:12`, and `en-US` renders `1.5 GB`, `67%` and `3:59:12 PM`. Nothing
hard-codes US formats.

## Adding a new locale

1. Add the tag to `SUPPORTED_LOCALES` in `i18n/types.ts`.
2. Create `i18n/locales/<tag>.ts` typed as `Messages` (start from `en-US.ts`).
   Set `meta.languageName` to the language's own name.
3. Register it in `i18n/messages.ts`.
4. Extend `matchLocale()` in `i18n/detect.ts` with its language mapping.
5. Add keywords to the Rust `KeywordIntentResolver` so the new suggestions resolve.
6. Update the allow-list in the "not accidental copies of English" test if
   brand names or key caps legitimately match.
7. Run `pnpm typecheck && pnpm test`, then check the UI for overflow: the
   language picker, navigation, Settings rows, the state preview grid, the
   telemetry rail and command hints.

## Writing copy

- Keys describe purpose (`settings.language.label`), never the English text.
- Keep placeholders whole: put the full sentence in the resource, not fragments.
- Avoid constructions that need grammatical gender agreement with a
  placeholder, e.g. `Completado: {tool}` rather than `{tool} completado`.
- Don't shrink text to make a translation fit; let layouts wrap.
