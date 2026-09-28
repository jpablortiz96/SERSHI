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
light source.

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

### Surfaces (dark, blue-black undertone)

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
(.12) for emphasis, `strong` (.20) sparingly, `focus` = signal cyan.

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
│ ◉ SERSHI  Pre-alpha        ( Home · Activity · Settings )   ● Local core – □ × │  44 px title bar
├───────────────┬──────────────────────────────────────────┬───────────────┤
│ THIS COMPUTER │                ◯  core                   │ RECENT ACTIVITY│
│ OS · arch     │              READY                       │ ● event        │
│ PROCESSOR 18% │         Ready when you are.              │ │ time         │
│ ~~~sparkline  │                                          │ ● event        │
│ MEMORY 9.2/16 │          How can I help?                 │               │
│ ▬▬▬▬───────   │   ( suggestion ) ( suggestion )          │               │
│               │   ╭──────────────────────────────╮       │               │
│               │   │ ● Ask SERSHI…          🎙  ↑ │       │               │
│  264 px rail  │   ╰──────────────────────────────╯       │  264 px rail  │
└───────────────┴──────────────────────────────────────────┴───────────────┘
```

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

Behind everything: state-tinted radial light at the core's position, one large
violet glow drifting over 28 s, orbital guides, 5% SVG grain, and a vignette. One
composited transform loop in total.

## Iconography

16 px grid, 1.5 px stroke, round caps and joins, `currentColor`, no fills except
the mark. Icons label actions; they never replace text for important meaning.

## Interaction patterns

| Pattern               | Spec                                                                                  |
| --------------------- | ------------------------------------------------------------------------------------- |
| Hover                 | `surface.hover` wash or text brighten, `duration.fast`                               |
| Focus (keyboard)      | 2 px `border.focus` outline, 2 px offset — always visible                            |
| Command focus bloom   | Bar ring + 4 px halo + soft shadow in `--state-color`                                |
| Press                 | scale 0.94–0.97, `duration.instant`; release with `ease.spring`                      |
| Tooltip               | CSS `data-tip`, fade + 4 px rise, `duration.fast` / `ease.enter`                    |
| Disabled              | `text.disabled`, `not-allowed` cursor, and a reason (tooltip or label)               |
| Confirmation dialog   | Modal (`role="dialog"`, `aria-modal`), overlay surface, `radius.xl`, `shadow.e3`. States the action, the resolved subject and the risk in plain words; primary button names the action ("Close Notepad"), never "OK" / "Allow action?". **Cancel is focused first**; Escape cancels; focus is trapped and restored. A countdown (tabular numerals) and a shrinking meter show the time left. Nothing in it comes from request text or a model |
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

A theme is a partial token override (`createTheme({ state: { listening: … } })`)
applied with `applyTheme()`. Themes can restyle but not invent tokens. Future
package format ([ROADMAP](ROADMAP.md) v0.8):

```text
theme/
├── manifest.json      id, name, version, author, sershi compatibility
├── tokens.json        partial token overrides
├── preview.webp
├── background.webp    optional ambient layer image
└── assets/
```

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
