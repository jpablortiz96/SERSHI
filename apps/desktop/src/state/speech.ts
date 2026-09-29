/**
 * Spoken replies. SERSHI's replies are phrased in the interface language by
 * the same code that writes them in the transcript (`composeReply`), then
 * spoken natively by the core with a local Windows voice. Output only: a
 * spoken reply cannot trigger anything.
 *
 * InterfaceAudio (UI cues) and speech are separate: turning cues off never
 * silences speech, and the other way round. While a spoken reply is due or
 * playing, cues that would duplicate it (e.g. the success chime) are held
 * back — see `speechExpected` and audio/connect.ts.
 */
import type { CommandOutcome } from "@sershi/contracts";

import { createTranslator } from "../i18n/translate";
import { createFormatters } from "../i18n/format";
import { composeReply } from "../i18n/domain";
import { useLocaleStore } from "../i18n/store";
import { desktopRuntime, sershi } from "../ipc";

/** Longest reply sent for speech (mirrors the core's limit). */
const MAX_SPEECH_CHARS = 600;
/** How long a promised reply may keep cues quiet if it never starts. */
const EXPECTATION_MS = 4_000;

let expectedUntil = 0;

/** A spoken reply is due or playing: skip cues that would duplicate it. */
export function speechExpected(now = Date.now()): boolean {
  return now < expectedUntil;
}

/** Marks a spoken reply as coming (called when SERSHI heard the user). */
export function expectSpeech(now = Date.now()): void {
  expectedUntil = now + EXPECTATION_MS;
}

export function clearSpeechExpectation(): void {
  expectedUntil = 0;
}

function i18n() {
  const locale = useLocaleStore.getState().locale;
  return { locale, t: createTranslator(locale), format: createFormatters(locale) };
}

/** Speaks text already phrased in the interface language. */
export function speakText(text: string): void {
  if (!desktopRuntime) return;
  const trimmed = text.trim().slice(0, MAX_SPEECH_CHARS);
  if (!trimmed) return;
  const { locale } = i18n();
  sershi.speakReply(trimmed, locale).catch(() => {
    clearSpeechExpectation();
  });
}

/** Speaks a command outcome the way the transcript shows it. */
export function speakOutcome(outcome: CommandOutcome): void {
  const { t, format } = i18n();
  speakText(composeReply(t, format, outcome));
}

/** Speaks a fixed voice notice. */
export function speakMessage(key: "voice.noSpeech" | "voice.unclear"): void {
  speakText(i18n().t(key));
}

export function stopSpeaking(): void {
  clearSpeechExpectation();
  if (desktopRuntime) sershi.stopSpeaking().catch(() => undefined);
}
