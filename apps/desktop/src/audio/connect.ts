/**
 * When SERSHI makes a sound. Only meaningful moments — start-up, summon,
 * an outcome, an approval request — never hovers, clicks or typing.
 *
 * Runs in the Command Center only, so one event never plays twice across
 * windows. Reads the *real* assistant state: previewing a state in Settings
 * is silent.
 */
import type { AssistantState } from "@sershi/contracts";

import { sershi } from "../ipc";
import { useAssistantStore } from "../state/assistant";
import type { Cue } from "./cues";
import { playCue } from "./interfaceAudio";

/** Which cue, if any, a state change should play. */
export function cueForTransition(from: AssistantState, to: AssistantState): Cue | null {
  if (from === to) return null;
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
    const cue = cueForTransition(previous, next);
    previous = next;
    if (cue) playCue(cue);
  });
  const stopSummon = sershi.onFocusCommand(() => {
    playCue("summon");
  });
  return () => {
    unsubscribe();
    stopSummon();
  };
}
