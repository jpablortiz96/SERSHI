# Sound design

SERSHI's interface sounds are a small, quiet vocabulary for a few meaningful
moments. They are **off by default**, supplementary to what is on screen, and
never part of any security decision.

> Status: implemented in Gate 2B. Playback on Windows (WebView2 audio,
> autoplay policy, perceived loudness) is REQUIRES_WINDOWS_VALIDATION.

## Principles

- **Rare.** Only start-up, summon, completed actions, errors and approval
  requests. Never hovers, clicks, typing, navigation or telemetry.
- **Quiet and short.** Default volume 35 %, a master ceiling of 0.55 behind
  a compressor. Start-up ≈ 0.95 s; every other cue ≤ 0.5 s.
- **Clean, calm and slightly spatial.** Sine and triangle partials tuned to
  open fifths and octaves, soft attacks, airy band-passed noise, gentle stereo
  movement. It should suggest intelligence and technology, not alarms or games.
- **Original.** Everything is synthesised procedurally. SERSHI uses no
  samples and imitates no franchise, film or operating-system sound.
- **Hierarchy.** Start-up is distinctive, summon and success are subtle,
  error is restrained (low and falling, never harsh), and confirmation is
  clear but calm (two soft pings that invite a decision without alarm).
- **Supplementary.** Everything a sound signals is already visible: the core
  state, the label and glyph, and the confirmation window. Muting loses no
  information.

## Events

| Cue            | When                                                        | Character                                             |
| -------------- | ----------------------------------------------------------- | ----------------------------------------------------- |
| `startup`      | The Command Center's first live connection, once per session | A low soft impact, rising open-fifth energy, then a small spatial shimmer (≈ 0.95 s) |
| `summon`       | Shortcut, tray, companion or second launch brings SERSHI forward (`sershi://focus-command`) | A quick upward glide (≈ 0.2 s) |
| `success`      | The real assistant state becomes `success`                   | Two soft rising notes, a fifth apart (≈ 0.3 s)         |
| `error`        | The real assistant state becomes `error`                     | A short, low, falling tone (≈ 0.3 s)                   |
| `confirmation` | The real assistant state becomes `awaitingConfirmation`      | Two gentle bell-like pings (≈ 0.45 s)                  |

Cues play only from the Command Center (`src/audio/connect.ts`), so an event
never sounds twice across windows. They read the **real** snapshot state:
previewing a state in Settings → Developer is silent. The confirmation window
imports no audio code, and a test enforces this.

## Procedural approach

- `src/audio/cues.ts` holds the sound set **as data**: `Tone` (wave, pitch
  glide, start, duration, peak, attack, pan, optional low-pass) and `Noise`
  (band-passed burst with pan). The whole language is a few hundred bytes.
- `src/audio/interfaceAudio.ts` renders a cue with Web Audio:
  - oscillator or noise → optional filter → envelope gain → `StereoPannerNode`
  - then master gain (volume × 0.55) → `DynamicsCompressorNode` → output
- **No idle cost.** The `AudioContext` is created lazily on the first cue and
  suspended about 400 ms after each cue ends. While sounds are off, or the
  volume is 0, no context is created at all.

## Settings

Settings → Appearance:

- **Interface sounds:** On / Off (default Off).
- **Sound volume:** 0–100 in steps of 5 (default 35), with **Play sample**.

Both persist in the presentation preferences (`sershi.preferences.v1`).

## Autoplay and limitations

SERSHI does **not** weaken WebView or Tauri security to force playback: no
autoplay flags, no command-line switches, no native audio path.

- If WebView2 refuses to start an `AudioContext` without a user gesture, the
  cue is skipped silently.
- The start-up cue plays when the Command Center first connects. If Windows
  refuses to play audio at that moment, the start-up cue is simply not heard;
  later cues work after any interaction.

Loudness varies with device and driver, which is why volume is user-controlled.

## Accessibility

- Off by default; one switch disables every cue.
- Sounds never carry information that is not also shown visually.
- Security never depends on audio. A confirmation is equally safe muted, and
  the approval window is identical with or without sound.
- Reduced motion does not mute sound; they are independent preferences.

## InterfaceAudio vs SpeechAudio

Interface cues (`InterfaceAudio`, `src/audio/`, Web Audio in the WebView)
and speech (`SpeechAudio`, Prompt 3: native capture and playback in Rust,
[VOICE.md](VOICE.md)) are separate systems:

| | Interface sounds | Speech |
| --- | --- | --- |
| Setting | Appearance › Interface sounds (off by default) | Voice › Voice responses (on by default for spoken requests), Speak typed responses (off) |
| Engine | Procedural Web Audio in the Command Center | Windows speech synthesis, played by SERSHI's WASAPI output |
| Volume | Interface volume slider | Windows voice/system volume |

- Turning interface sounds off never silences speech, and turning speech
  off never silences cues.
- Speech does not reuse the cue mixer.

**Ducking rules** (`audio/connect.ts`):

- No cue while SERSHI is Listening, Transcribing or Speaking; the summon cue
  never plays over the microphone.
- The success and error cues are skipped when a spoken reply is due. The
  reply says it already.
- The approval-request cue always plays: it asks for a human decision on
  the trusted surface.
- The start-up cue never triggers or reaches capture: the microphone is off
  at start-up.

## Future sound packs

A sound pack is **data**: a `SoundSet` of procedural parameters and/or audio
assets (validated formats, size-limited). It is never executable code, never
JavaScript, never a plugin. A pack can change how SERSHI sounds; it cannot
add events, change when cues play, or reach any IPC command.

Packs are less privileged than Skills (see [SECURITY.md](SECURITY.md#personalization-is-not-privilege)).
