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
| `duration.breath`     | 5600 ms | Ambient breathing loop                               |
| `duration.drift`      | 28 s    | Ambient drift, field ring rotation                   |

Interactive durations never exceed 720 ms (enforced by a unit test).

| Easing           | Curve                          | Use                                            |
| ---------------- | ------------------------------ | ---------------------------------------------- |
| `ease.standard`  | `cubic-bezier(.2, 0, 0, 1)`    | General movement, color                        |
| `ease.enter`     | `cubic-bezier(.16, 1, .3, 1)`  | Things arriving (messages, views, ripples)     |
| `ease.exit`      | `cubic-bezier(.4, 0, 1, 1)`    | Things leaving                                 |
| `ease.spring`    | `cubic-bezier(.34, 1.4, .64, 1)` | Press release, error contraction (soft overshoot) |
| `ease.breathe`   | `cubic-bezier(.45, 0, .55, 1)` | Symmetric ambient loops                        |

## The Intelligent Core

A layered, procedural renderer (`components/core/Core.tsx`) driven only by
`data-state`:

| Layer       | Geometry                                | Motion                                          |
| ----------- | --------------------------------------- | ----------------------------------------------- |
| `halo`      | Radial light, 168% of core              | Breath: scale 0.94↔1.04, opacity 0.72↔0.92     |
| `field`     | 1 px ring, 230° visible arc             | Rotation, 28 s                                  |
| orbit A     | Ring tilted rotateX 72° / rotateY −16°  | Rotation, 16 s, with a bright satellite         |
| orbit B     | Violet ring tilted 66° / 42°            | Reverse rotation, 23 s                          |
| `cognition` | Segmented ring (dashes × 150° arc)      | Visible when thinking/planning; 3.6 s           |
| `drive`     | Bright 100° comet arc                   | Visible when executing; 1.15 s                  |
| `pulses`    | Three concentric rings                  | Listening: gather inward 2.4 s · Speaking: radiate outward 1.6 s |
| `body`      | Luminous sphere, frost → state hue      | Breath: scale 1↔1.035                           |
| `nucleus`   | White point                             | Static                                          |
| `flash`     | One-shot ring, re-keyed per revision    | Success/warning/error ripple                    |

### Companion renderers

`Core` takes `{ state, size, pulseKey }` and holds no state. Any future visual pack
(Rive, Lottie, sprite, 3D) implements the same props:

```text
AssistantState  →  Companion renderer (props only)  →  active visual pack
```

so assistant logic never depends on a particular avatar.

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
| Window open (v0.1)            | Content fades/rises 6 px over `slow`, `enter`                               |
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

REQUIRES_WINDOWS_VALIDATION: WebView2 GPU compositing of the transparent companion
and its idle CPU cost have not been measured on Windows yet.

## Reduced motion

With `prefers-reduced-motion: reduce`: all loops stop; ripples and contractions are
removed; state-specific geometry (dashed ring, drive arc, a single pulse ring)
remains visible statically; color changes shorten to `duration.fast`; view and
message transitions become near-instant. The state label beside the core always
names the state.

## Audio visualisation (v0.3)

The pulse layer is the reserved place for voice. In v0.3 its amplitude will be
driven by the real input/output level. It never animates unless the microphone or
speech output is actually active; the Listening visuals seen today are reachable
only through Developer Mode's labelled state preview.
