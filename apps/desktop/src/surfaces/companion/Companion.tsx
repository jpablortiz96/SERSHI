import { useEffect, useRef, type PointerEvent } from "react";

import { Core } from "../../components/core/Core";
import { STATE_COPY } from "../../components/core/stateCopy";
import { currentWindow, sershi } from "../../ipc";
import { connectAssistant, useAssistantStore, useDisplayState } from "../../state/assistant";
import styles from "./Companion.module.css";

/** Pointer travel (px) that turns a press into a window drag. */
const DRAG_THRESHOLD = 4;

/**
 * The floating companion. Renders the shared assistant state and does exactly
 * two things: a click summons the Command Center, a drag moves the window.
 * Its capability file grants it nothing else.
 */
export function Companion() {
  const state = useDisplayState();
  const revision = useAssistantStore((s) => s.snapshot.revision);
  const press = useRef<{ x: number; y: number } | null>(null);

  useEffect(connectAssistant, []);

  const summon = () => {
    sershi.summonCommandCenter().catch(() => undefined);
  };

  const onPointerDown = (e: PointerEvent) => {
    if (e.button === 0) press.current = { x: e.clientX, y: e.clientY };
  };
  const onPointerMove = (e: PointerEvent) => {
    const start = press.current;
    if (start && Math.hypot(e.clientX - start.x, e.clientY - start.y) > DRAG_THRESHOLD) {
      press.current = null;
      currentWindow.startDragging();
    }
  };
  const onPointerUp = () => {
    if (press.current) summon();
    press.current = null;
  };

  return (
    <main className={styles.stage} data-state={state}>
      <button
        type="button"
        className={styles.companion}
        aria-label={`Open SERSHI Command Center. SERSHI is ${STATE_COPY[state].label.toLowerCase()}.`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => (press.current = null)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") summon();
        }}
      >
        <Core state={state} size={128} pulseKey={revision} />
      </button>
    </main>
  );
}
