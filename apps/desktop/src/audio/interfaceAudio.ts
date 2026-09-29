/**
 * InterfaceAudio — SERSHI's non-voice UI sounds.
 *
 * Separate from speech (microphone, speech-to-text, text-to-speech; native,
 * see docs/VOICE.md): turning interface sounds off never silences spoken
 * replies, and speech has its own settings and pipeline. Ducking rules live
 * in connect.ts.
 *
 * Idle cost is zero: no AudioContext exists until the first cue, and the
 * context is suspended again shortly after each cue ends, so nothing runs
 * while SERSHI is quiet. When sounds are off, nothing is created at all.
 *
 * Sound is supplementary. No state, decision or security behaviour depends
 * on it; every cue has a visual equivalent.
 */
import { useAppearance } from "../visual/appearance";
import { cueDuration, SERSHI_SOUNDS, type Cue, type Noise, type SoundSet, type Tone } from "./cues";

/** Master level at 100 % volume: interface sounds stay well below speech. */
const MASTER_CEILING = 0.55;
/** Suspend the context this long after the last cue ends. */
const IDLE_SUSPEND_MS = 400;

type AudioContextFactory = () => AudioContext;

let factory: AudioContextFactory | null = () => new AudioContext({ latencyHint: "interactive" });
let context: AudioContext | null = null;
let master: GainNode | null = null;
let noiseBuffer: AudioBuffer | null = null;
let suspendTimer: ReturnType<typeof setTimeout> | undefined;
let busyUntil = 0;
const soundSet: SoundSet = SERSHI_SOUNDS;

/** Tests replace the AudioContext; `null` simulates a runtime without Web Audio. */
export function setAudioContextFactory(next: AudioContextFactory | null): void {
  factory = next;
  context = null;
  master = null;
  noiseBuffer = null;
}

function ensureContext(): AudioContext | null {
  if (context) return context;
  if (!factory || typeof globalThis.AudioContext === "undefined") return null;
  try {
    context = factory();
    master = context.createGain();
    const limiter = context.createDynamicsCompressor();
    limiter.threshold.value = -12;
    limiter.ratio.value = 6;
    master.connect(limiter).connect(context.destination);
  } catch {
    context = null;
  }
  return context;
}

function noiseSource(ctx: AudioContext): AudioBuffer {
  if (!noiseBuffer) {
    const length = Math.floor(ctx.sampleRate * 0.6);
    noiseBuffer = ctx.createBuffer(1, length, ctx.sampleRate);
    const data = noiseBuffer.getChannelData(0);
    for (let i = 0; i < length; i++) data[i] = Math.random() * 2 - 1;
  }
  return noiseBuffer;
}

function envelope(ctx: AudioContext, at: number, voice: Tone | Noise): GainNode {
  const gain = ctx.createGain();
  const { peak, attack, duration } = voice;
  gain.gain.setValueAtTime(0.0001, at);
  gain.gain.linearRampToValueAtTime(peak, at + attack);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + duration);
  return gain;
}

function panner(ctx: AudioContext, at: number, voice: Tone | Noise): StereoPannerNode {
  const pan = ctx.createStereoPanner();
  pan.pan.setValueAtTime(voice.pan, at);
  if (voice.panTo !== undefined) pan.pan.linearRampToValueAtTime(voice.panTo, at + voice.duration);
  return pan;
}

function schedule(ctx: AudioContext, out: AudioNode, t0: number, voice: Tone | Noise): void {
  const at = t0 + voice.start;
  const gain = envelope(ctx, at, voice);
  const pan = panner(ctx, at, voice);
  let source: AudioScheduledSourceNode;
  let head: AudioNode;
  if (voice.kind === "tone") {
    const osc = ctx.createOscillator();
    osc.type = voice.wave;
    osc.frequency.setValueAtTime(voice.from, at);
    if (voice.to !== voice.from) {
      osc.frequency.exponentialRampToValueAtTime(voice.to, at + voice.duration);
    }
    source = osc;
    head = osc;
    if (voice.lowpass) {
      const filter = ctx.createBiquadFilter();
      filter.type = "lowpass";
      filter.frequency.value = voice.lowpass;
      head = osc.connect(filter);
    }
  } else {
    const buffer = ctx.createBufferSource();
    buffer.buffer = noiseSource(ctx);
    const filter = ctx.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.value = voice.band;
    filter.Q.value = voice.q;
    source = buffer;
    head = buffer.connect(filter);
  }
  head.connect(gain).connect(pan).connect(out);
  source.start(at);
  source.stop(at + voice.duration + 0.05);
}

function scheduleSuspend(ctx: AudioContext): void {
  clearTimeout(suspendTimer);
  const wait = Math.max(0, busyUntil - Date.now()) + IDLE_SUSPEND_MS;
  suspendTimer = setTimeout(() => {
    if (ctx.state === "running") ctx.suspend().catch(() => undefined);
  }, wait);
}

/** Whether a cue would play right now (sounds enabled, volume above zero). */
export function cuesEnabled(): boolean {
  const { interfaceSounds, soundVolume } = useAppearance.getState();
  return interfaceSounds && soundVolume > 0;
}

/**
 * Plays a cue if interface sounds are on. Returns whether playback was
 * requested. Safe to call anywhere: failures (no audio device, a runtime
 * that refuses audio without a user gesture) are silent — sound is never
 * required.
 */
export function playCue(cue: Cue): boolean {
  if (!cuesEnabled()) return false;
  const ctx = ensureContext();
  if (!ctx || !master) return false;
  const volume = useAppearance.getState().soundVolume / 100;
  master.gain.value = volume * MASTER_CEILING;
  const out = master;
  const start = () => {
    const t0 = ctx.currentTime + 0.01;
    for (const voice of soundSet[cue]) schedule(ctx, out, t0, voice);
    busyUntil = Math.max(busyUntil, Date.now() + cueDuration(soundSet, cue) * 1000);
    scheduleSuspend(ctx);
  };
  if (ctx.state === "running") {
    start();
  } else {
    // Resuming can be refused without a user gesture (autoplay policy); the
    // cue is then skipped rather than forced.
    ctx
      .resume()
      .then(() => {
        if (ctx.state === "running") start();
      })
      .catch(() => undefined);
  }
  return true;
}
