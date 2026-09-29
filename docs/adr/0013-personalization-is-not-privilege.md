# 0013 — Personalization is not privilege

**Status:** Accepted · 2026-09

## Context

Gate 2B adds four personalization features:

- a configurable global shortcut (the default collided with another application during physical testing)
- System / Light / Dark themes
- interface sounds
- the foundation for future companion appearances

Each of them could quietly become a path to authority. A shortcut could
approve something. A theme could need a permission. A downloaded "companion"
could run code. Sound could become part of a confirmation.

## Decision

- **Presentation state stays presentation state.** Theme, motion, companion
  appearance and size, sounds and volume, and the user's chosen shortcut
  live in the WebView's presentation preferences (`sershi.preferences.v1`).
  Nothing in Rust reads them to decide what SERSHI may do.
- **One new command, one window.** `set_global_shortcut` is granted only to
  the Command Center. It takes a string, and Rust validates it
  (`sershi_core::shortcut`):
  - two or more modifiers, including Ctrl or Alt
  - the key is A–Z, 0–9, Space or F1–F12
  - never Win
  Rust registers it before releasing the previous one. The shortcut
  _summons_ SERSHI; it approves nothing.
- **No silent substitution.** If the requested or stored shortcut is
  unavailable, the previous one stays. With none, the status reads
  Unavailable. SERSHI never picks a different combination on its own.
- **Themes are token overrides.** Components use semantic CSS variables
  only. There is no theme-specific logic in components, and switching themes
  rewrites variables without re-rendering.
- **Sounds are data plus a renderer.** Cues are procedural parameters; the
  audio engine plays them. Security never depends on audio, and the
  confirmation window imports no audio code.
- **Companion appearances are pure renderers** of `AssistantState`
  (`CompanionRenderer`). Unknown appearances fall back to Orbital.
- **Future packs** (companion, theme, sound) are **data**: images, sprites,
  vector assets, animation metadata, tokens, audio. They are never
  executable code, and they are less privileged than Skills.
- **CI guards it.** Contract tests check that:
  - only the Command Center holds `set_global_shortcut`
  - no command is named after theme, sound, appearance or packs
  - the confirmation window keeps exactly its two commands
  - the decision payload is unchanged

## Alternatives

- **Rust-owned preferences store.** This would make theme and sound state
  visible to the authority layer, which is the wrong direction. It is
  deferred to the v0.1 settings store, and even then presentation keys stay
  separate from policy.
- **Automatic fallback shortcut when the default is taken.** Rejected: the
  user would not know which keys summon SERSHI, and the fallback might break
  another application silently.
- **Rust theme adapter (Windows `AppsUseLightTheme`).** This is not needed
  while `prefers-color-scheme` works in WebView2. It is kept as the fallback
  if physical validation shows otherwise.
- **Sample-based sounds.** Rejected: licensing, size, and harder to theme as
  data.

## Consequences

- Personalization can change how SERSHI looks, sounds and is summoned.
  It cannot change what SERSHI is authorized to do.
- The confirmation window follows the theme only through CSS variables and
  the stored preference. It still has exactly two commands and cannot listen
  to events.
- Future marketplace work must keep packs declarative and parse them as
  untrusted data.
