# Design system

SERSHI should feel like an intelligent presence in the operating system — not a
dashboard, not a chat website. This document is the contract that lets any
engineer build new surfaces that look like they belong.

Source of truth: [`packages/design-tokens/src/tokens.ts`](../packages/design-tokens/src/tokens.ts).
Motion: [MOTION_SYSTEM.md](MOTION_SYSTEM.md). Brand: [BRAND.md](BRAND.md).

![Command Center](assets/command-center.webp)

## Philosophy

1. **Presence over panels.** The core is the protagonist. Everything else is
   supporting information arranged around it.
2. **Light, not decoration.** Depth comes from light the core casts (tinted by
   state), hairlines and shadow — not boxes, gradients-for-their-own-sake or neon.
3. **State is color.** One hue per assistant state, shared by the companion, the
   ambient light, the command bar focus and status labels.
4. **Restraint.** Near-black, cool neutrals, one accent at a time. Warmth (ember)
   is rare and means "attention".
5. **Honesty.** Nothing looks active that is not. Unavailable features say so.

**Mood:** a dark observatory at night — deep, calm, precise, with one intelligent
light source. SERSHI Light keeps the same mood in cool daylight: an
instrument in a quiet, bright room, where the core is still the only light.

## How tokens reach CSS

`applyTheme()` writes every token to `:root` as a CSS custom property at start-up
(through the CSSOM, compatible with the strict CSP). Components use only variables.

| Token path              | CSS variable              |
| ----------------------- | ------------------------- |
| `color.signalDeep`      | `--color-signal-deep`     |
| `surface.raised`        | `--surface-raised`        |
| `type.hero.size`        | `--type-hero-size`        |
| `motion.duration.fast`  | `--duration-fast`         |
| `motion.ease.enter`     | `--ease-enter`            |
| `state.listening`       | `--state-listening`       |

`--state-color` is special: a registered `@property` (`<color>`) set by
`[data-state]` on any surface, so a state change **interpolates** across every
element that derives from it. Derive tints with `color-mix(in oklab, var(--state-color) N%, transparent)`.

## Color

### Surfaces (SERSHI Dark, blue-black undertone; see [Themes](#themes) for Light)

| Token             | Value                     | Use                                  |
| ----------------- | ------------------------- | ------------------------------------ |
| `surface.base`    | `#07080b`                 | Window background                    |
| `surface.raised`  | `#0b0d12`                 | Rails, stage floor                   |
| `surface.overlay` | `#10131a`                 | Inputs, popovers, tooltips           |
| `surface.hover`   | `rgb(255 255 255 / .04)`  | Hover wash                           |
| `surface.active`  | `rgb(255 255 255 / .07)`  | Selected / pressed                   |
| `surface.glass`   | `rgb(16 19 26 / .72)`     | The only translucent surface (command bar) |

### Text

| Token            | Value      | Contrast on base | Use                          |
| ---------------- | ---------- | ---------------- | ---------------------------- |
| `text.primary`   | `#eef3f8`  | 17.9:1           | Content, values              |
| `text.secondary` | `#a3adbd`  | 8.8:1            | Supporting copy              |
| `text.tertiary`  | `#767f91`  | 5.0:1 (≥ 4.6:1 on every surface) | Labels, metadata, hints |
| `text.disabled`  | `#454c5a`  | 2.3:1            | Disabled controls only (WCAG-exempt); never for information |

### Palette and state hues

| State        | Token             | Hue                      | Meaning                          |
| ------------ | ----------------- | ------------------------ | -------------------------------- |
| sleeping     | `state.sleeping`  | `#5c6a80` slate          | Dormant                          |
| idle         | `state.idle`      | `#9fdfff` ice            | Present, calm                    |
| awake        | `state.awake`     | `#cfefff` pale ice       | Attending to the user            |
| listening    | `state.listening` | `#56d4f5` signal cyan    | Receiving input                  |
| thinking     | `state.thinking`  | `#9a8cff` aura violet    | Understanding                    |
| planning     | `state.planning`  | `#b7a9ff` soft violet    | Sequencing steps                 |
| executing    | `state.executing` | `#5aa9ff` volt blue      | Acting                           |
| speaking     | `state.speaking`  | `#7fe6f2` aqua           | Responding                       |
| success      | `state.success`   | `#5fe3b0` mint           | Completed                        |
| warning      | `state.warning`   | `#f2b870` ember          | Needs attention (rare warmth)    |
| awaitingConfirmation | `state.awaitingConfirmation` | `#ecc98a` soft gold | Waiting for the user's approval — warm like warning, but paler and calmer: a question, not a problem |
| error        | `state.error`     | `#f2777a` coral          | Failed                           |
| offline      | `state.offline`   | `#6b7486`                | Condition: no connectivity       |
| private      | `state.private`   | `#c9d2de`                | Condition: mic/screen off        |

The cool family (ice → cyan → blue → violet) carries "intelligence"; warm hues are
reserved for attention and failure so they never lose meaning.

### Borders

`border.faint` (.05) separates regions, `subtle` (.08) outlines controls, `default`
(.12) for emphasis, `strong` (.20) sparingly, `focus` = signal cyan (amber
`state.awaitingConfirmation` inside the confirmation window).

### Atmosphere and core materials

Themeable semantic tokens so a theme can re-light SERSHI without touching
components: `atmosphere.primary` (slow background glow, aura violet),
`atmosphere.secondary` (cool floor light), `atmosphere.floor`,
`atmosphere.vignette`; `core.specular`, `core.inner`, `core.orbit`,
`core.halo` (halo strength). State colour still comes from `state.*`.

## Typography

**Geist Variable** for display and UI, **Geist Mono Variable** for ids, times and
numbers in tables. Both are bundled (OFL-1.1); SERSHI makes no font requests at
runtime. Fallbacks: Segoe UI Variable → Segoe UI → system-ui; Cascadia Mono → Consolas.

| Role      | Size / line | Weight | Tracking | Use                                   |
| --------- | ----------- | ------ | -------- | ------------------------------------- |
| `hero`    | 44 / 48     | 300    | −0.035em | One greeting per screen               |
| `display` | 30 / 36     | 320    | −0.025em | Page titles                           |
| `title`   | 20 / 26     | 480    | −0.012em | Section titles in dense views         |
| `heading` | 15 / 22     | 560    | −0.005em | Row headings                          |
| `body`    | 14 / 21     | 400    | 0        | Default                               |
| `small`   | 13 / 19     | 420    | 0.002em  | Secondary content                     |
| `caption` | 12 / 16     | 450    | 0.01em   | Metadata                              |
| `label`   | 10.5 / 14   | 600    | 0.14em, UPPERCASE | Section labels ("THIS COMPUTER") |
| `mono`    | 12 / 18     | 420    | 0, tabular | Ids, times, durations               |
| `metric`  | 26 / 28     | 300    | −0.03em, tabular | Telemetry values              |

Rules: large text is light, small text is heavier. Keep prose under
`layout.readable` (68ch). Replies in the transcript use 15.5/24 for comfortable
reading. Never use letter-spaced uppercase for more than a few words.

## Space, radius, depth

- **Spacing:** 4 px grid (`space.1` = 4 … `space.20` = 80). Rails use 32 px vertical
  rhythm between groups, 8–12 px within.
- **Radii:** `xs` 4 · `sm` 8 · `md` 12 · `lg` 16 · `xl` 22 · `2xl` 28 · `pill`.
  Controls that sit on their own (command bar, chips, nav) are pills; content rows
  have no radius.
- **Shadows:** `e1` controls, `e2` floating inputs/tooltips, `e3` dialogs. Each
  includes a 1 px inset top highlight — surfaces are lit from above.
- **Blur:** `md` (16 px) on the command bar only. Glass is a privilege, not a style.
- **Z-index:** `base` 0 · `raised` 10 · `sticky` 100 · `overlay` 1000 · `modal` 1100 ·
  `toast` 1200 · `tooltip` 1300.

## Layout: the orbital stage

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ ◉ SERSHI  Pre-alpha   ( ⌂ Home · ∿ Activity · ⚙ Settings )  ● Local core – □ × │  44 px title bar
├───────────────┬──────────────────────────────────────────┬───────────────┤
│ THIS COMPUTER │                ◯  core                   │ RECENT ACTIVITY│
│ OS · arch     │              READY                       │ ● event        │
│ PROCESSOR 18% │         Ready when you are.              │ │ time         │
│ ~~~sparkline  │                                          │ ✓ event        │
│ MEMORY 9.2/16 │          How can I help?                 │               │
│ ▬▬▬▬───────   │   ( suggestion ) ( suggestion )          │               │
│               │   ╭──────────────────────────────╮       │               │
│               │   │ ● Ask SERSHI…          🎙  ↑ │       │               │
│  264 px rail  │   ╰──────────────────────────────╯       │  264 px rail  │
└───────────────┴──────────────────────────────────────────┴───────────────┘
```

- The **core is the light source**: the ambient light and orbital guides follow
  its measured position (`--core-y`).
- **Responsive:** below 1120 px wide the rails narrow to 224 px; below 720 px
  high the core scales to 80 % and the greeting to display size; at 900 px and
  taller the stage is capped at 860 px and centred. Minimum window 960 × 640.
- The **stage** (centre, ≤ 640 px content) holds presence → dialogue → input.
  When a conversation starts the core shrinks (scale 0.6) and the transcript takes
  the space; the greeting disappears.
- **Rails** are columns of text separated by space and type hierarchy — no card
  borders. Telemetry is intentionally quiet: SERSHI is not a system monitor.
- **Orbital guides** — three faint concentric rings centred on the core — tie the
  composition together.
- Secondary views (Activity, Settings) are a single readable column with sections
  separated by hairlines.

## Ambient layer

Behind everything: state-tinted light centred on the core (local — it never
repaints the whole window), one large `atmosphere.primary` glow drifting over
28 s, a faint cool floor, orbital guides, 5% SVG grain and a vignette. One
composited transform loop in total.

## Iconography

16 px grid, 1.5 px stroke, round caps and joins, `currentColor`, no fills except
the mark (`components/shell/icons.tsx`: navigation, window controls, send,
voice, refresh, approval mark). Icons label actions; they never replace text
for important meaning.

**State and status glyphs** (12 px, `components/core/StateGlyph.tsx`): one
shape per assistant state (dot, focus, segments, nodes, arrow, check,
triangle, hourglass, cross, waves, hollow) and per activity tone. They sit
beside a text label so meaning never depends on colour (colour blindness).

## Interaction patterns

| Pattern               | Spec                                                                                  |
| --------------------- | ------------------------------------------------------------------------------------- |
| Hover                 | `surface.hover` wash or text brighten, `duration.fast`                               |
| Focus (keyboard)      | 2 px `border.focus` outline, 2 px offset — always visible                            |
| Command focus bloom   | Bar ring + 4 px halo + soft shadow in `--state-color`                                |
| Press                 | scale 0.94–0.98, `duration.instant`; release with `ease.spring` (the companion releases without overshoot) |
| Navigation            | Icon + label; the selected backdrop slides between tabs; active icon takes the state tint |
| Radio choices         | Language, motion and companion size share one pattern: bordered options, selected ring, keyboard arrows |
| Tooltip               | CSS `data-tip`, fade + 4 px rise, `duration.fast` / `ease.enter`                    |
| Disabled              | `text.disabled`, `not-allowed` cursor, and a reason (tooltip or label)               |
| Confirmation window   | Approval mark ("SERSHI approval" + shield), amber hairline frame and amber focus ring. Its own compact 440×320 window (not a browser alert, message box or in-page modal): overlay surface with a faint state-colored glow, SERSHI mark and wordmark, "Confirmation required" label, the question as the title ("Close Notepad?"), plain-words body and risk, reason, countdown (tabular numerals) and shrinking meter. Primary button names the action ("Close Notepad"), never "OK" / "Allow action?". **Cancel is focused first**; Escape and × cancel. Approve fades in over a 600 ms arming delay. Risk colour: sensitive = amber, high risk = stronger coral accent on the caution line; red is not used for ordinary confirmations. Nothing in it comes from request text or a model |
| Candidate choices     | When a name is ambiguous, the reply lists candidates as secondary buttons; choosing one re-asks with the exact name |

## Accessibility

- Keyboard: `/` focuses the command bar, `Enter` sends, `↑` recalls, `Esc` clears or
  dismisses, `Ctrl+1…3` switches views. Every control is a real `button`.
- Screen readers: the core is `role="img"` with the state label; the status line is
  `role="status"`; SERSHI's replies are in an `aria-live="polite"` list; meters use
  `role="meter"`.
- Contrast: every informational text token meets WCAG AA (4.5:1) on every
  surface; `text.disabled` is reserved for disabled controls.
- Reduced motion: see [MOTION_SYSTEM.md](MOTION_SYSTEM.md#reduced-motion).
- Voice is never required; every action has a keyboard path.

## Localization and layout

Copy comes from `src/i18n` ([LOCALIZATION.md](LOCALIZATION.md)). Spanish and
Portuguese strings are often noticeably longer than English, so layouts wrap instead of shrinking
type: rails and row headers use `flex-wrap`, Settings values wrap under their
labels, hint groups never break internally, and the greeting uses
`text-wrap: balance`. Language names in the picker are endonyms (English,
Español, Português) so anyone can find their own.

## Themes

Settings → Appearance → Theme offers **System** (default), **Light** and
**Dark**. The registry is in `apps/desktop/src/visual/themes.ts`, and the
token overrides are in `packages/design-tokens/src/themes.ts`.

- **SERSHI Dark** (`darkTheme = {}`) is the base token set: everything
  documented above.
- **SERSHI Light** is a complete, separate design, not an inversion.

A theme is a partial token override (`createTheme(overrides)`) applied with
`applyTheme()`. Themes can restyle but not invent tokens; a test checks that
Light defines exactly the same keys as Dark. Components never branch on the
theme. They read semantic variables, and a test fails if a component
compares `theme === "dark"`.

`connectAppearance()` (`src/visual/appearance.ts`) resolves the preference
and rewrites the variables **only when the resolved theme changes**. It also
sets `html[data-theme]` and `color-scheme`, so switching never recreates or
re-renders the app.

**System** follows `prefers-color-scheme` live. It never uses the clock.
Every window follows the same preference, synchronised by the `storage`
event; the confirmation window reads the stored preference and `matchMedia`,
without any new permission.

### SERSHI Light

The Light theme uses soft cool daylight, not a white page.

| Token                                          | Light value                  | Note                                   |
| ---------------------------------------------- | ---------------------------- | -------------------------------------- |
| `surface.base` / `raised` / `overlay`          | `#eceff4` / `#f3f5f8` / `#f8f9fb` | Off-white and cool grey, never `#fff` |
| `text.primary`                                 | `#101827`                    | 15.4:1 on base (≥ 15.4:1 on every surface) |
| `text.secondary`                               | `#3e4a5c`                    | 7.8:1                                  |
| `text.tertiary`                                | `#586475`                    | 5.2:1                                  |
| `text.accent`                                  | `#0a6d8f`                    | 5.1:1                                  |
| `text.disabled`                                | `#9aa3b1`                    | Disabled only (WCAG-exempt)            |
| `action.primary` / `onPrimary`                 | `#101827` / `#f8f9fb`        | Ink-dark primary button, 16.9:1        |
| `border.focus`                                 | `#0a7aa6`                    | ≥ 3:1 on every surface                 |
| `state.*`                                      | deeper hues (idle `#2b8fc2`, thinking `#6b5bdc`, success `#138f6b`, awaitingConfirmation `#a8792a`, error `#cf3f47` …) | Same meanings, ≥ 3:1 non-text contrast |
| `atmosphere.*`, `core.*`                       | a quiet cool wash, soft vignette, white specular | The core stays the light source, with a controlled glow |
| `shadow.*`                                     | softer, lower-opacity        |                                        |

Tokens added in Gate 2B so that no component hard-codes a colour:

| Token                 | Dark                     | Use                                     |
| --------------------- | ------------------------ | --------------------------------------- |
| `surface.tint`        | `rgb(255 255 255 / .02)` | Faint panel fill (title bar, transcript) |
| `border.highlight`    | `rgb(255 255 255 / .06)` | Top highlight on raised glass           |
| `border.shade`        | `rgb(0 0 0 / .4)`        | Bottom shade / inner edge               |
| `action.primary`      | `#eef3f8`                | Primary button (send, confirm)          |
| `action.onPrimary`    | `#07080b`                | Label on the primary button             |
| `atmosphere.guide`    | `rgb(255 255 255 / .035)`| Orbital guide rings                     |
| `atmosphere.grain`    | `0.05`                   | Film-grain opacity                      |

Accessibility tests (`packages/design-tokens/test/tokens.test.ts`) run on
**both** themes:

- text ≥ 4.5:1 on every surface
- the primary action ≥ 4.5:1
- focus and state hues ≥ 3:1

Future theme packages are **data only** ([ROADMAP](ROADMAP.md) v0.8, [ADR 0013](adr/0013-personalization-is-not-privilege.md)):

```text
theme/
├── manifest.json      id, name, version, author, sershi compatibility
├── tokens.json        partial token overrides (validated against the token keys)
├── preview.webp
├── background.webp    optional ambient layer image
└── assets/
```

No scripts, no CSS injection, no permissions.

## Do / Don't

| Do                                                            | Don't                                                          |
| ------------------------------------------------------------- | -------------------------------------------------------------- |
| Use tokens and `color-mix` on `--state-color`                 | Hard-code hex values or durations in components                |
| Separate with space, type and hairlines                       | Wrap everything in bordered cards                              |
| Let the core be the only glowing object                       | Add glowing borders to panels and buttons                      |
| Use ember/coral only for attention/failure                    | Use warm colors decoratively                                   |
| Say "Planned for v0.3" on unavailable features                | Show fake data, fake waveforms, or controls that do nothing silently |
| Keep one translucent surface per screen                       | Stack blurred glass layers                                     |
| Use Geist light for large text, mono for ids/times            | Use futuristic display fonts that hurt readability             |
| Keep telemetry small and neutral                              | Make CPU graphs the hero of the screen                          |
| Reference SERSHI's own geometry (orbits, core)                | Imitate arc reactors, HUD reticles, Tron grids or RGB gamer styling |
