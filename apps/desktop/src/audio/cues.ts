/**
 * SERSHI's interface sound language, as data. Every cue is synthesised at
 * play time from these parameters (oscillators, envelopes, filtered noise,
 * stereo placement) — no samples, nothing copyrighted, a few hundred bytes.
 * A future sound pack is a different `SoundSet`: parameters, never code.
 *
 * Character: clean sine/triangle partials tuned to open fifths and
 * octaves, soft attacks, airy filtered-noise shimmer, gentle stereo width.
 * Quiet by design; see docs/SOUND_DESIGN.md.
 */

export type Cue = "startup" | "summon" | "success" | "error" | "confirmation";

export const CUES: readonly Cue[] = ["startup", "summon", "success", "error", "confirmation"];

/** A pitched partial. Times in seconds from the cue's start. */
export interface Tone {
  kind: "tone";
  wave: "sine" | "triangle";
  from: number;
  to: number;
  start: number;
  duration: number;
  /** Peak gain before the master volume (0–1). */
  peak: number;
  /** Seconds to reach the peak; the rest is an exponential release. */
  attack: number;
  /** -1 (left) … 1 (right); `panTo` moves it during the note. */
  pan: number;
  panTo?: number;
  /** Optional low-pass cutoff (Hz) to soften the partial. */
  lowpass?: number;
}

/** A short burst of band-passed noise (impact body or air/shimmer). */
export interface Noise {
  kind: "noise";
  start: number;
  duration: number;
  peak: number;
  attack: number;
  /** Band-pass centre frequency (Hz) and Q. */
  band: number;
  q: number;
  pan: number;
  panTo?: number;
}

export type Voice = Tone | Noise;

export type SoundSet = Record<Cue, readonly Voice[]>;

const tone = (t: Omit<Tone, "kind">): Tone => ({ kind: "tone", ...t });
const noise = (n: Omit<Noise, "kind">): Noise => ({ kind: "noise", ...n });

/** The default SERSHI sound set. */
export const SERSHI_SOUNDS: SoundSet = {
  /** ~0.95 s — something intelligent coming online: a soft low impact,
      rising open-fifth energy, then a small spatial shimmer. */
  startup: [
    tone({
      wave: "sine",
      from: 72,
      to: 54,
      start: 0,
      duration: 0.42,
      peak: 0.42,
      attack: 0.012,
      pan: 0,
      lowpass: 400,
    }),
    noise({ start: 0, duration: 0.14, peak: 0.12, attack: 0.004, band: 180, q: 0.8, pan: 0 }),
    tone({
      wave: "triangle",
      from: 196,
      to: 294,
      start: 0.08,
      duration: 0.62,
      peak: 0.1,
      attack: 0.28,
      pan: -0.35,
      panTo: -0.15,
      lowpass: 2200,
    }),
    tone({
      wave: "triangle",
      from: 294,
      to: 440,
      start: 0.12,
      duration: 0.6,
      peak: 0.08,
      attack: 0.3,
      pan: 0.35,
      panTo: 0.15,
      lowpass: 2600,
    }),
    tone({
      wave: "sine",
      from: 880,
      to: 880,
      start: 0.5,
      duration: 0.42,
      peak: 0.05,
      attack: 0.02,
      pan: -0.5,
      panTo: 0.5,
    }),
    tone({
      wave: "sine",
      from: 1320,
      to: 1320,
      start: 0.58,
      duration: 0.36,
      peak: 0.035,
      attack: 0.02,
      pan: 0.45,
      panTo: -0.4,
    }),
    noise({
      start: 0.46,
      duration: 0.48,
      peak: 0.03,
      attack: 0.12,
      band: 7200,
      q: 2.5,
      pan: -0.6,
      panTo: 0.6,
    }),
  ],
  /** ~0.22 s — subtle: a quick upward glide, "I'm here". */
  summon: [
    tone({
      wave: "sine",
      from: 587,
      to: 880,
      start: 0,
      duration: 0.2,
      peak: 0.12,
      attack: 0.02,
      pan: -0.1,
      panTo: 0.1,
    }),
    tone({
      wave: "sine",
      from: 1175,
      to: 1760,
      start: 0.02,
      duration: 0.16,
      peak: 0.03,
      attack: 0.02,
      pan: 0.2,
    }),
  ],
  /** ~0.3 s — subtle: two soft rising notes (a fifth). */
  success: [
    tone({
      wave: "triangle",
      from: 587,
      to: 587,
      start: 0,
      duration: 0.16,
      peak: 0.1,
      attack: 0.008,
      pan: -0.15,
      lowpass: 3000,
    }),
    tone({
      wave: "triangle",
      from: 880,
      to: 880,
      start: 0.07,
      duration: 0.24,
      peak: 0.09,
      attack: 0.008,
      pan: 0.15,
      lowpass: 3200,
    }),
  ],
  /** ~0.3 s — restrained: a short, low, falling tone. Never harsh. */
  error: [
    tone({
      wave: "sine",
      from: 330,
      to: 247,
      start: 0,
      duration: 0.28,
      peak: 0.13,
      attack: 0.01,
      pan: 0,
      lowpass: 1200,
    }),
    tone({
      wave: "sine",
      from: 165,
      to: 147,
      start: 0.02,
      duration: 0.24,
      peak: 0.07,
      attack: 0.01,
      pan: 0,
      lowpass: 800,
    }),
  ],
  /** ~0.45 s — clear but calm: two gentle bell-like pings that invite a
      decision without alarm. */
  confirmation: [
    tone({
      wave: "sine",
      from: 659,
      to: 659,
      start: 0,
      duration: 0.3,
      peak: 0.1,
      attack: 0.006,
      pan: -0.2,
    }),
    tone({
      wave: "sine",
      from: 988,
      to: 988,
      start: 0.12,
      duration: 0.34,
      peak: 0.08,
      attack: 0.006,
      pan: 0.2,
    }),
    tone({
      wave: "sine",
      from: 1976,
      to: 1976,
      start: 0.12,
      duration: 0.2,
      peak: 0.015,
      attack: 0.006,
      pan: 0.2,
    }),
  ],
};

/** Length of a cue in seconds. */
export function cueDuration(set: SoundSet, cue: Cue): number {
  return Math.max(...set[cue].map((v) => v.start + v.duration));
}
