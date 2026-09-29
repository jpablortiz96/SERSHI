/**
 * When SERSHI makes a sound. Only meaningful moments — start-up, summon,
 * an outcome, an approval request — never hovers, clicks or typing.
 *
 * Runs in the Command Center only, so one event never plays twice across
 * windows. Reads the *real* assistant state: previewing a state in Settings
 * is silent.
 *
 * Ducking (interface sounds vs speech): no cue plays while SERSHI listens,
 * transcribes or speaks, and the success cue is skipped when a spoken reply
 * is about to say the same thing. The approval cue always plays: it asks
 * for a human decision on the trusted surface.
 */
import type { AssistantState } from "@sershi/contracts";

import { sershi } from "../ipc";
import { useAssistantStore } from "../state/assistant";
import { speechExpected } from "../state/speech";
import type { Cue } from "./cues";
import { playCue } from "./interfaceAudio";

const VOICE_STATES: readonly AssistantState[] = ["listening", "transcribing", "speaking"];

/** Which cue, if any, a state change should play. */
export function cueForTransition(
  from: AssistantState,
  to: AssistantState,
  speechDue = false,
): Cue | null {
  if (from === to) return null;
  if (VOICE_STATES.includes(to)) return null;
  // A spoken reply says it already.
  if (speechDue && (to === "success" || to === "error")) return null;
  switch (to) {
    case "success":
      return "success";
    case "error":
      return "error";
    case "awaitingConfirmation":
      return "confirmation";
    default:
      return null;
  }
}

const STARTUP_KEY = "sershi.startupCuePlayed";

/** Plays the start-up cue once per SERSHI session (not on every reload). */
function playStartupOnce(): void {
  try {
    if (sessionStorage.getItem(STARTUP_KEY)) return;
    sessionStorage.setItem(STARTUP_KEY, "1");
  } catch {
    // No session storage: play at most once for this page.
  }
  playCue("startup");
}

/** Connects interface sounds to SERSHI's events. Returns a cleanup. */
export function connectInterfaceSounds(): () => void {
  let previous = useAssistantStore.getState().snapshot.state;
  let startupPending = true;
  const unsubscribe = useAssistantStore.subscribe((s) => {
    if (startupPending && s.connection === "live") {
      startupPending = false;
      playStartupOnce();
    }
    const next = s.snapshot.state;
    const cue = cueForTransition(previous, next, speechExpected());
    previous = next;
    if (cue) playCue(cue);
  });
  const stopSummon = sershi.onFocusCommand(() => {
    // Never over the microphone.
    if (!VOICE_STATES.includes(useAssistantStore.getState().snapshot.state)) playCue("summon");
  });
  return () => {
    unsubscribe();
    stopSummon();
  };
}
