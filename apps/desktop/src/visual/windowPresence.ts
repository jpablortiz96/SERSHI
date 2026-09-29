/**
 * Window enter / exit transitions for the Command Center.
 *
 * Rust shows and hides the native window; this hook only animates the
 * content around those moments: a short fade-and-settle when the window
 * appears (open, summon from tray, shortcut, companion or a second launch —
 * one transition for every origin), and a quick retreat before the ×
 * button asks Rust to hide it. With reduced motion both are skipped.
 */
import { tokens } from "@sershi/design-tokens";
import { useCallback, useEffect, useRef, useState } from "react";

import { sershi } from "../ipc";
import { motionReduced } from "./appearance";

export type WindowPresence = "entering" | "present" | "leaving";

const ms = (value: string) => Number.parseFloat(value);
export const WINDOW_ENTER_MS = ms(tokens.motion.duration.window);
export const WINDOW_EXIT_MS = ms(tokens.motion.duration.windowExit);
/** A summon right after another does not replay the entrance. */
const REPLAY_GUARD_MS = 1200;

export function useWindowPresence(): { presence: WindowPresence; hide: () => void } {
  const [presence, setPresence] = useState<WindowPresence>("entering");
  const lastEntrance = useRef(0);
  const timer = useRef<number | undefined>(undefined);

  const enter = useCallback(() => {
    const now = Date.now();
    window.clearTimeout(timer.current);
    if (motionReduced() || now - lastEntrance.current < REPLAY_GUARD_MS) {
      setPresence("present");
      return;
    }
    lastEntrance.current = now;
    setPresence("entering");
    timer.current = window.setTimeout(() => {
      setPresence("present");
    }, WINDOW_ENTER_MS);
  }, []);

  useEffect(() => {
    enter();
    const onVisibility = () => {
      if (document.visibilityState === "visible") enter();
    };
    document.addEventListener("visibilitychange", onVisibility);
    const stopFocus = sershi.onFocusCommand(enter);
    return () => {
      window.clearTimeout(timer.current);
      document.removeEventListener("visibilitychange", onVisibility);
      stopFocus();
    };
  }, [enter]);

  const hide = useCallback(() => {
    // Rust hides the window and cancels any pending approval; SERSHI keeps
    // running in the companion and tray.
    const request = () => {
      sershi.hideCommandCenter().catch(() => undefined);
    };
    window.clearTimeout(timer.current);
    if (motionReduced()) {
      request();
      return;
    }
    setPresence("leaving");
    timer.current = window.setTimeout(() => {
      request();
      // Ready for the next appearance, whether or not the platform reports
      // a visibility change.
      timer.current = window.setTimeout(() => {
        setPresence("present");
      }, WINDOW_EXIT_MS);
    }, WINDOW_EXIT_MS);
  }, []);

  return { presence, hide };
}
