# 0012 — State-driven visual system

**Status:** Accepted · 2026-09

## Context

Prompt 2 makes the Command Center, the companion and the confirmation
window feel like one living product. Without a single rule, each surface
would style assistant states its own way, reduced motion would drift
between modules, and future themes would have to touch components.

## Decision

- **One table** (`apps/desktop/src/visual/stateVisuals.ts`) maps every
  assistant state to a motion pattern, energy, busy/attention flags and a
  glyph. The Core, the companion's presence and the state glyphs read it.
- **One procedural renderer** (`components/core/Core.tsx`) with variants
  (`hero`, `companion`, `compact`, `preview`), CSS-only: layered elements
  animating transform / translate / opacity. No canvas, WebGL or animation
  library.
- **One motion switch:** `<html data-motion="reduced">`, set from the
  Appearance preference or the OS; stylesheets key on it instead of their
  own media queries.
- **Themeable semantics:** `atmosphere.*` and `core.*` tokens plus a theme
  registry; themes are partial token overrides.
- **Contextual presence** uses only SERSHI's own state and a Rust
  `sershi://presence` event — never information about the user's screen.

## Alternatives

- **Framer Motion / Motion:** ~30–50 kB for orchestration CSS already does;
  JS-driven loops would cost main-thread time in an always-on window.
- **Canvas / WebGL core:** richer effects, but continuous rendering in the
  always-on companion and a separate reduced-motion path.
- **Per-component state styling:** simplest short term; drifts immediately.

## Consequences

- Adding a state means one table row (tests fail until it exists).
- Visual packs can replace the Core by implementing the same props.
- Visual work cannot widen authority: contract tests pin every window's
  capabilities (Gate 1A invariants).
