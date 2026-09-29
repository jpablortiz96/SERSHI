# Visual Experience

How SERSHI looks and moves across its three surfaces, and why. The design
tokens live in [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md); timing and easing in
[MOTION_SYSTEM.md](MOTION_SYSTEM.md). This document covers the living system
that ties them together (Prompt 2, "Visual Experience 2.0").

| Surface             | Role                                           | Authority |
| ------------------- | ---------------------------------------------- | --------- |
| Command Center      | Where the user thinks with SERSHI              | Requests actions; cannot approve |
| Floating companion  | Where SERSHI exists when the Command Center is gone | Summon only |
| Confirmation window | Where the human keeps authority                | The only approval surface |

Visual work never changes these roles. Tests in
`packages/contracts/test/commands.test.ts` ("Gate 1A invariants survive
visual work") fail CI if a capability widens.

## One source of truth for state

```text
AssistantState (Rust)
   → visual/stateVisuals.ts  { pattern, energy, busy, attention, glyph }
   → Core (data-pattern, --energy)   → hero · companion · compact · preview
   → companion presence (companionIntensity)
   → ambient light (state colour, core position)
   → state glyph next to every state label
```

Colour comes from the `state.*` tokens via `data-state`; everything else —
motion pattern, light output, shape — comes from `STATE_VISUALS`. No component
styles a state on its own. A test asserts the table covers every state.

## The Core (renderer 2.0)

One CSS-only renderer (`components/core/Core.tsx`), no canvas, WebGL or
animation library. Layers, back to front:

| Layer      | What it is                               | Moves in              |
| ---------- | ---------------------------------------- | --------------------- |
| bloom      | light cast on the surroundings (hero)    | static, fades with energy |
| halo       | soft outer light                         | breath (rest: ambient 9.6 s) |
| field      | broken outer arc                         | 28 s drift            |
| orbits a/b | tilted paths + satellite                 | 16 s / 23 s; planes re-tilt per state |
| compute    | two counter-rotating segmented rings     | thinking              |
| structure  | four arcs that assemble + nodes          | planning              |
| drive      | one directional sweep                    | executing             |
| resonance  | concentric rings                         | listening (in) / speaking (out) |
| motes      | three barely visible specks              | resting only          |
| shell      | thin energy shell around the body        | attend, drive, bloom, await |
| body       | the core, nucleus and a soft rim light   | breath + sub-pixel drift |
| flash      | one-shot outcome ring                    | bloom, caution, falter |

Variants use the same layers: **hero** (Command Center, casts light),
**companion** (contained glow so the transparent window never shows a
square), **compact** (inline marks; orbits omitted), **preview** (static
thumbnails for the state preview). The core is abstract by rule: no face,
no eyes — a single bright point, never two.

### State motion language

| State | Pattern | What you see | Shape |
| ----- | ------- | ------------ | ----- |
| Sleeping | dormant | dimmed, nothing moves | hollow ring |
| Ready | rest | slow breath, motes, sub-pixel drift — noticeable only if you look | dot |
| Attending | attend | core gathers, halo tightens, orbits align, shell closes in | focus |
| Listening | receive | rings gather inward (visual only; no microphone exists) | waves |
| Thinking | compute | two segmented rings counter-rotate, light compresses | segments |
| Planning | structure | four arcs swing into square order, nodes appear, orbits hold still | nodes |
| Working | drive | one sweep, energy passes outward through the shell | arrow |
| Speaking | resonate | rings radiate outward (visual only; no TTS exists) | waves |
| Done | bloom | 3–4 % expansion, one soft ring, settle | check |
| Needs attention | caution | slow amber pulse | triangle |
| Waiting for you | await | steady amber, nothing spins, shell closes | hourglass |
| Couldn't complete | falter | contracts, orbits lose alignment, brief coral ring | cross |

Listening and Speaking exist for the future voice layer and are shown only in
the state preview; SERSHI never animates as if it were listening or watching.

## Companion 2.0

- **Size:** visible core 88 / 104 / 120 CSS px (Settings → Appearance →
  Companion size) inside the unchanged 176 px transparent window; the hit
  target is a 136 px circle. CSS pixels scale with Windows DPI.
- **Hover:** the halo strengthens (`--hover`), the nucleus leans a hair
  (≤ 1.5 px) towards the pointer. No scale bounce.
- **Press:** 4 % compression, released without overshoot. A click summons.
- **Drag:** beyond 4 px the press becomes a native window drag; the core
  shrinks 7 %, loops pause, and it settles back when the drag ends. A drag
  never summons.
- **Contextual presence** (`companionIntensity`): calm while the Command
  Center is on screen, normal when the companion is SERSHI's only sign,
  present while busy or when something needs the user. Rust tells the
  companion whether the Command Center is visible with the
  `sershi://presence` event — an event, not a command; the companion's
  capability is unchanged.
- **Attention policy:** only outcomes and approvals change the companion
  abruptly (bloom, falter, amber await). Idle stays quiet.
- **Deferred:** edge-aware composition and edge magnetism. Both risk moving
  or reshaping the companion without clear intent; they need physical
  Windows feedback first.

## Command Center 2.0

```text
┌──────────────────────────────────────────────────────────────┐
│ SERSHI  pre-alpha      [⌂ Home  ∿ Activity  ⚙ Settings]   ● ─□×│
│                                                              │
│ THIS COMPUTER          ( core: the light source )    RECENT  │
│ OS · CPU · memory        ◉ READY / status line       ✓ …      │
│                          response (latest emphasised) △ …      │
│                          ─────── command bar ───────          │
└──────────────────────────────────────────────────────────────┘
```

- **Depth, not boxes:** background → atmosphere (drift, floor, vignette,
  grain) → core light → stage → interactive surfaces → confirmation window.
- **Core as light source:** the ambient light and orbital guides follow the
  core's measured position (`--core-y`), so the room stays lit from the core
  at every window size and when the conversation starts.
- **Navigation:** icon + label, a selection indicator that slides between
  tabs (measured, so long labels in every language fit).
- **State line:** glyph + label + one short line; never colour alone.
- **Command bar:** focus bloom in the state colour; typing brightens its
  signal; on submit the command lifts away (520 ms) while the input clears,
  so the request is visibly accepted before the assistant state takes over;
  while busy a thin light travels along its lower edge.
- **Responses:** the latest exchange is the response (larger, full
  contrast); earlier ones recede to 50 % and return on hover or focus.
- **Activity:** every row has a shape per tone (✓ △ ✕ ○ •) and the page has
  a Status column with a word, so meaning never depends on colour.
- **Window transitions:** one entrance for every origin (open, tray,
  shortcut, companion, second launch): opacity 0 → 1 and scale 0.985 → 1 in
  260 ms; not replayed within 1.2 s. × plays a 160 ms retreat, then Rust
  hides the native window. Reduced motion skips both.

### Responsive behaviour

| Window | Behaviour |
| ------ | --------- |
| Minimum 960 × 640 | rails narrow to 224 px; core scales to 80 %, hero text to display size |
| Default 1200 × 780 | reference layout |
| Tall (≥ 900 px) | the stage is capped at 860 px and centred, so presence, response and command bar stay one composition |

## Confirmation window

Visually part of SERSHI, deliberately calmer than the Command Center:
brand, an **approval mark** (a small shield around the core with the words
"SERSHI approval"), a quiet amber hairline frame, an amber focus ring, then
the question, the plain-words risk, the countdown and Cancel / Action. The
mark is visual consistency only; it is not an operating-system identity
signal and does not imply Windows Hello or an enclave. Behaviour is
unchanged from Gate 1A.

## Appearance and themes

- **Settings → Appearance** shows only options that work:
  - Theme (System / Light / Dark)
  - Motion (System / Reduced)
  - Companion (Orbital, read-only)
  - Companion size
  - Interface sounds (On / Off)
  - Sound volume
  These are presentation preferences, stored with the language preference
  and shared by every window. They never change what SERSHI may do
  ([ADR 0013](adr/0013-personalization-is-not-privilege.md)).
- **Why no "Full" motion option:** it would have to override the operating
  system's reduced-motion accessibility setting; "System" already gives full
  motion when Windows allows it.
- **Themes (Gate 2B):** SERSHI Dark and SERSHI Light are both token sets
  (`packages/design-tokens/src/themes.ts`). System follows Windows' app mode
  live through `prefers-color-scheme`. The Command Center, companion and
  confirmation window all follow the theme. The confirmation window stays
  recognisable: same layout, approval mark and amber frame, with deeper
  amber in Light. See [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md#themes).
- **Interface sounds:** see [SOUND_DESIGN.md](SOUND_DESIGN.md).

## Companion appearances

```text
AssistantState ──► CompanionRenderer (props only) ──► OrbitalRenderer   (today)
                                                  └─► future renderers
```

- `src/visual/companions.tsx` is the registry. A `CompanionRenderer` is
  `{ id, label, component }`, and the component receives
  `{ state, size, intensity, pulseKey }`: nothing else, no IPC, no stores.
- **Orbital** (the Core in its `companion` variant) is the only renderer
  and the only option in Settings. An unknown stored appearance falls back
  to Orbital. SERSHI shows no placeholder options such as "coming soon".
- **Future renderers:** Aurora, Minimal, Holographic, Character2D. Each only
  maps the assistant state to visuals.

### Future character companions

A character companion is another renderer. It might use frame sets per
visual state or a skeletal/vector rig driven by animation metadata:

- idle and attending
- thinking and working
- success and error
- speaking

The rule stays:

```text
AssistantState ──► companion visual state      (never the other way round)
```

A character has **no business logic of its own**. It cannot decide, speak,
act, approve or request anything. It shows the state Rust already
broadcast, and changing appearance never changes SERSHI's powers.

### Companion packs (future, not built)

A downloadable pack is **data, not code**:

- images, sprites, vector assets
- animation metadata (frame ranges per state, timings)
- theme tokens
- audio assets or procedural sound parameters
- a manifest

Never "download companion → execute JavaScript". Packs will be parsed as
untrusted input (size-limited, schema-validated), rendered by SERSHI's own
renderer, and granted no commands or permissions. They are less privileged
than Skills (see [SECURITY.md](SECURITY.md#personalization-is-not-privilege)).
There is no marketplace yet.

## Reduced motion

One switch: `<html data-motion="reduced">`, set from the Appearance choice or
the operating system. Every looping animation is neutralised under it (a
test scans all stylesheets); the Core keeps a distinct static shape per
state (segmented rings, four arcs with nodes, the sweep arc, a closed shell,
misaligned orbits) plus colour, glyph and label.

## Performance

- CSS-only, transform / translate / opacity animations; no new dependencies.
- Bundle (production build): main JS 26.7 → 31.0 kB, shared chunk
  287.1 → 293.4 kB, CSS 41.7 → 53.6 kB (all surfaces), companion JS
  1.0 → 1.8 kB.
- Idle CPU under the Linux virtual display (software rendering, no GPU
  compositing — every animated frame is painted by the CPU, so absolute
  numbers are far above what WebView2 on Windows will show):

  | Scenario | Gate 1A baseline | Prompt 2 |
  | -------- | ---------------- | -------- |
  | Command Center visible, idle | 133.8 % of one core | 133.3 % |
  | Companion only, idle | 32.9 % | 31.8 % |

  Same conditions, same machine, 20 s windows: no regression. The < 1 % idle
  target is REQUIRES_WINDOWS_VALIDATION (Task Manager, Gate 2A).
- How to inspect on Windows: Task Manager → Details → `sershi-desktop.exe`
  and its `msedgewebview2.exe` children, with the Command Center hidden and
  SERSHI idle for 30 s.

## Screenshots

`docs/assets/`: `command-center.webp` (Home, idle, Spanish, real app),
`p2-home-thinking.webp`, `p2-home-executing.webp`, `p2-home-success.webp`,
`p2-home-large.webp`, `p2-home-minimum-pt-reduced.webp` (960 × 640,
Portuguese, reduced motion), `p2-settings.webp`,
`p2-settings-state-preview.webp`, `p2-activity-confirmation.webp` (real app,
Spanish), `companion-states.webp` (all 12 states), `p2-companion-desktop.webp`,
`p2-confirmation-{en,es,pt}.webp`. Captured on Linux (Chromium preview and
WebKitGTK under Xvfb); Windows rendering is pending Gate 2A.

Gate 2B (Chromium preview, light system mode): `g2b-light-home.webp`,
`g2b-light-thinking.webp`, `g2b-light-settings-appearance.webp`,
`g2b-light-confirmation.webp`, `g2b-light-companion.webp`.
