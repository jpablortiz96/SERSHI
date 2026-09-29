# Motion system

Motion is how SERSHI communicates state before the user reads a word. It must be
calm, meaningful and cheap enough to run all day.

![Companion states](assets/companion-states.webp)

## Principles

1. **Motion encodes state.** Every loop means something (breathing = alive,
   orbiting = thinking, pulses = hearing/speaking). Nothing moves just to move.
2. **Calm by default.** Idle motion is slow (5.6 s breath, 16–28 s orbits). Speed
   rises only while SERSHI is actually working.
3. **Compositor only.** Animate `transform` and `opacity`. Colors change through the
   registered `--state-color` transition. No animated layout, blur or shadow.
4. **Never re-time a running loop.** State-specific layers fade in from opacity 0;
   ambient loops keep constant durations and are _paused_, never sped up, so state
   changes never cause a visual jump.
5. **Never required.** Every state has a distinct label and color; reduced motion
   removes loops without losing meaning.

## Tokens

| Token                 | Value   | Use                                                  |
| --------------------- | ------- | ---------------------------------------------------- |
| `duration.instant`    | 80 ms   | Press feedback                                       |
| `duration.fast`       | 160 ms  | Hover, tooltips, small color changes                 |
| `duration.default`    | 240 ms  | Most UI transitions, focus bloom                     |
| `duration.slow`       | 420 ms  | View changes, message arrival, layer fades           |
| `duration.cinematic`  | 720 ms  | State re-tint, core resize, success ripple           |
| `duration.window`     | 260 ms  | Command Center appearing (every summon origin)       |
| `duration.windowExit` | 160 ms  | Command Center retreating before the native hide     |
| `duration.breath`     | 5600 ms | Ambient breathing loop                               |
| `duration.ambient`    | 9600 ms | Idle life: the resting halo, sub-pixel drift          |
| `duration.drift`      | 28 s    | Ambient drift, field ring rotation                   |

Interactive durations never exceed 720 ms and window transitions 280 ms
(enforced by a unit test). Components never hard-code durations except the
few documented pattern loops above.

| Easing           | Curve                          | Use                                            |
| ---------------- | ------------------------------ | ---------------------------------------------- |
| `ease.standard`  | `cubic-bezier(.2, 0, 0, 1)`    | General movement, color                        |
| `ease.enter`     | `cubic-bezier(.16, 1, .3, 1)`  | Things arriving (messages, views, ripples)     |
| `ease.exit`      | `cubic-bezier(.4, 0, 1, 1)`    | Things leaving                                 |
| `ease.spring`    | `cubic-bezier(.34, 1.4, .64, 1)` | Press release, error contraction (soft overshoot) |
| `ease.breathe`   | `cubic-bezier(.45, 0, .55, 1)` | Symmetric ambient loops                        |

## The Intelligent Core

A layered, procedural renderer (`components/core/Core.tsx`) driven by the
state → visual table (`visual/stateVisuals.ts`, [ADR 0012](adr/0012-state-driven-visual-system.md)).
Layers, patterns and the full state motion language are documented in
[VISUAL_EXPERIENCE.md](VISUAL_EXPERIENCE.md#the-core-renderer-20). Summary:

| Pattern (states) | Motion |
| ---------------- | ------ |
| dormant (sleeping) | nothing moves, 42 % opacity |
| rest (idle) | halo breath over `ambient` (9.6 s), motes drift over `drift`, body sub-pixel drift over 2 × `ambient` |
| attend (awake) | orbit planes re-tilt into alignment over `cinematic`; halo tightens; shell closes in |
| compute (thinking) | two segmented rings, 3.6 s and 5.4 s, counter-rotating; shell compresses |
| structure (planning) | four arcs assemble (3.2 s cycle, 80 ms stagger), nodes appear at lock, orbits pause |
| drive (executing) | sweep arc 1.3 s; shell passes outward 1.3 s |
| receive / resonate | rings gather 2.4 s / radiate 1.6 s; shell scale follows the real audio level (≤ 7 %) |
| bloom (success) | one ripple + 4 % shell expansion over `cinematic`, once |
| caution (warning) | halo attention 2.4 s |
| await (awaitingConfirmation) | halo attention 3.2 s; orbits and field pause |
| falter (error) | body contracts to 0.9 then 0.97, orbit planes misalign, one coral ripple |

### Companion renderers

`Core` takes `{ state, variant, size, intensity, pulseKey }` and holds no state. Any future visual pack
(Rive, Lottie, sprite, 3D) implements the same props:

```text
AssistantState  →  Companion renderer (props only)  →  active visual pack
```

so assistant logic never depends on a particular avatar.

Since Gate 2B this contract is the `CompanionRenderer` registry
(`src/visual/companions.tsx`; Orbital today). See
[VISUAL_EXPERIENCE.md](VISUAL_EXPERIENCE.md#companion-appearances).

### Theme changes

Switching theme (or Windows changing app mode under **System**) rewrites the
token variables once. There is no animation, no re-render and no window
recreation. Colours that already transition (`--state-color`) follow their
normal durations. Theme never affects motion; reduced motion stays
independent.

## State transitions

Colors always cross-fade over `duration.cinematic` with `ease.standard`.

| Transition               | Visual intent                        | Duration / easing                     | Effects                                                                         | Reduced motion                         |
| ------------------------ | ------------------------------------ | ------------------------------------- | ------------------------------------------------------------------------------- | -------------------------------------- |
| sleeping → idle          | Waking up gently                     | `slow` opacity, `cinematic` color     | Core opacity 0.42 → 1; paused loops resume from where they stopped              | Instant opacity; color fade `fast`     |
| idle → awake             | Turning attention to the user        | `cinematic`, `standard`               | Halo brightens to full; hue shifts to pale ice                                  | Color only                             |
| awake → listening        | Opening to input                     | `slow` layer fade                     | Pulses fade in and gather inward (2.4 s)                                        | One static ring at 1.35×               |
| listening → thinking     | Taking the input in                  | `slow` fades                          | Pulses fade out; cognition ring fades in and spins; hue → violet                | Static dashed ring                     |
| thinking → planning      | Organising                           | `cinematic` color                     | Cognition reverses direction; hue lightens                                      | Static dashed ring, lighter hue        |
| planning → executing     | Committing to action                 | `default` fade                        | Cognition out, drive arc in (fast comet); hue → volt blue                       | Static arc                             |
| executing → speaking     | Delivering                           | `slow` fade                           | Drive out; pulses radiate outward (1.6 s); hue → aqua                          | One static ring                        |
| speaking → idle          | Settling back                        | `slow`, `cinematic` color             | Pulses fade; breathing continues                                                | Color only                             |
| any → success            | Quiet confirmation                   | `cinematic`, `enter`                  | Single mint ripple (scale 1 → 2.3, fade out); holds 2.4 s then settles         | No ripple; mint hue + "Done" label     |
| any → warning            | Needs attention, not alarm           | 2.4 s loop, `breathe`                 | Ember ripple, halo pulses slowly; holds 4 s                                     | Ember hue + label                      |
| planning → awaitingConfirmation | Pausing for the user, patiently | 3.2 s loop, `breathe`                 | Hue → soft gold; halo pulses slowly (slower than warning); orbits pause; the confirmation window opens | Gold hue + "Waiting for you" label; static halo |
| awaitingConfirmation → executing | Permission granted            | `cinematic` color                     | Orbits resume; drive arc in; hue → volt blue                                    | Color only                             |
| any → error              | Clear but not dramatic               | `slow`, `spring`                      | Body contracts to 0.86 and recovers; coral ripple; orbits pause                 | Coral hue + label, no contraction      |
| error → idle             | Recovery                             | `cinematic` color                     | Orbits resume, hue returns to ice                                               | Color only                             |

Settle timing lives in the Rust shell (`runtime::schedule_settle`) and is discarded
if a newer request changed the state first. `awaitingConfirmation` does not
settle on a timer; it ends with the user's decision or the confirmation's expiry
(`runtime::schedule_expiry`).

## Command Center motion

| Moment                        | Spec                                                                        |
| ----------------------------- | --------------------------------------------------------------------------- |
| Window appears                | opacity 0 → 1, scale 0.985 → 1 over `window` (260 ms), `enter`; one entrance for every summon origin; not replayed within 1.2 s |
| Window hides (×)              | opacity → 0, scale 0.99, 4 px down over `windowExit` (160 ms), `exit`; then the native hide |
| Navigation                    | Selection indicator slides to the active tab over `slow`, `enter`           |
| Command accepted              | Sent text lifts 18 px and fades over 520 ms while the input clears          |
| Busy command bar              | A thin light travels along the bar's lower edge, 1.4 s                      |
| View change                   | Keyed view fades and rises 6 px over `slow`, `enter`                        |
| Conversation starts           | Core slot 300 → 168 px and core scale 0.6 over `cinematic`, `enter`         |
| Message arrives               | Opacity 0 → 1, translateY 8 → 0 over `slow`, `enter`                        |
| Activity entry arrives        | Opacity 0 → 1, translateX 6 → 0 over `slow`, `enter`                        |
| Command focus bloom           | Ring + halo in state color over `default`                                   |
| Sending                       | Signal dot pulses (1 s) while the core works                                |
| Memory meter                  | `scaleX` over `cinematic`                                                   |
| Ambient                       | 28 s drift, alternate; re-tint over `cinematic`                             |
| Confirmation window           | Content rises over `slow` with `enter`; no bounce. A 1 px state-colored line marks the top edge; the expiry meter shrinks linearly (`scaleX`); Approve fades from 55% to full opacity when armed (600 ms). Reduced motion: no rise, static meter |
| Notification (v0.1)           | From the companion: slides 8 px, holds, fades with `exit`                   |

## Performance

- Every loop is a `transform`/`opacity` animation on its own layer (`will-change`).
- Masks (conic gradients) are static relative to the rotating element, so rotation
  stays on the compositor.
- No `filter: blur()` in animations; glows are pre-baked radial gradients.
- Sleeping pauses every loop. The Command Center stops polling when hidden.
- Budget targets for v0.9 hardening: companion idle < 1% CPU on a mid-range laptop
  with WebView2; no long tasks > 50 ms during state transitions.

Measured under the Linux virtual display (software rendering): no idle CPU
regression versus Gate 1A — see [VISUAL_EXPERIENCE.md](VISUAL_EXPERIENCE.md#performance).

REQUIRES_WINDOWS_VALIDATION: WebView2 GPU compositing of the transparent companion
and its idle CPU cost have not been measured on Windows yet.

## Reduced motion

One switch, `<html data-motion="reduced">`, set from Settings → Appearance →
Motion or, with "System", from the operating system (`visual/appearance.ts`).
Under it: every animation and transition is cut to near-zero (global rule);
the Core shows a distinct static shape per pattern (segmented rings, four
arcs with nodes, the sweep arc, one resonance ring, a closed shell,
misaligned orbits); window and command transitions are skipped. The glyph
and label beside the core always name the state. A test scans every
stylesheet: any looping animation must be neutralised under the switch.

## Audio visualisation (Prompt 3)

Listening and Speaking follow real audio. The core sends a bounded 0–1 level
(`sershi://voice-level`):

- **Listening:** the microphone RMS.
- **Speaking:** the envelope of the samples SERSHI hands to the output
  device.

Levels arrive at most 25 times per second, with fast attack and slow release.
`visual/voiceLevel.ts` writes the level to `--voice-level` once per frame.
The Core's `receive` and `resonate` patterns scale the shell by at most 7 %
and brighten the resonance rings. The mic button's ring follows the input
level.

Raw audio never reaches the UI. With reduced motion the level is ignored;
colour, glyph and label carry the state.

- **Transcribing** (new state) reuses the `compute` pattern with its own
  colour (`state.transcribing`): the microphone is off and SERSHI is working
  on what it heard.
- Listening and Speaking are shown only while the microphone captures or
  speech plays. Developer Mode's labelled preview is the only exception.
