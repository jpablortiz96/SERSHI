import { useEffect, useRef, useState, type PointerEvent } from "react";

import { useI18n } from "../../i18n";
import { stateLabel } from "../../i18n/domain";
import { currentWindow, sershi } from "../../ipc";
import { connectAssistant, useAssistantStore, useDisplayState } from "../../state/assistant";
import { COMPANION_CORE_SIZE, useAppearance } from "../../visual/appearance";
import { companionRenderer } from "../../visual/companions";
import { companionIntensity } from "../../visual/stateVisuals";
import { connectVoiceLevel } from "../../visual/voiceLevel";
import styles from "./Companion.module.css";

/** Pointer travel (px) that turns a press into a window drag. */
export const DRAG_THRESHOLD = 4;
/** After a drag the OS owns the pointer; settle back if no event arrives. */
const DRAG_SETTLE_MS = 900;

/**
 * The floating companion: SERSHI's presence when the Command Center is gone.
 *
 * It renders the shared assistant state and does exactly two things: a
 * click summons the Command Center, a drag moves the window. Its capability
 * file grants it nothing else — it can never approve anything, and it never
 * sees audio or controls the microphone (it only receives a 0–1 level to
 * animate Listening and Speaking).
 *
 * Presence adapts to context SERSHI already owns: calmer while the Command
 * Center is on screen, more present when it is the only sign of SERSHI,
 * fully present while busy or when something needs the user.
 */
export function Companion() {
  const state = useDisplayState();
  const revision = useAssistantStore((s) => s.snapshot.revision);
  const size = useAppearance((s) => COMPANION_CORE_SIZE[s.companionSize]);
  // The appearance only changes pixels: the shell below (click, drag,
  // capability) is the same for every renderer.
  const Renderer = companionRenderer(useAppearance((s) => s.companionAppearance)).component;
  // The Command Center opens with SERSHI, so it starts visible.
  const [commandCenterVisible, setCommandCenterVisible] = useState(true);
  const [pressed, setPressed] = useState(false);
  const [dragging, setDragging] = useState(false);
  const press = useRef<{ x: number; y: number } | null>(null);
  const stage = useRef<HTMLElement>(null);
  const settle = useRef<number | undefined>(undefined);
  const { t } = useI18n();

  useEffect(connectAssistant, []);
  // Visual only: a bounded 0–1 level while listening or speaking.
  useEffect(connectVoiceLevel, []);
  useEffect(
    () =>
      sershi.onPresence(({ commandCenterVisible: visible }) => {
        setCommandCenterVisible(visible);
      }),
    [],
  );
  useEffect(
    () => () => {
      window.clearTimeout(settle.current);
    },
    [],
  );

  const intensity = companionIntensity(state, commandCenterVisible);

  const summon = () => {
    sershi.summonCommandCenter().catch(() => undefined);
  };

  const endDrag = () => {
    window.clearTimeout(settle.current);
    setDragging(false);
  };

  /** Pointer proximity: the core's light leans a hair towards the pointer. */
  const track = (e: PointerEvent) => {
    const el = stage.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const px = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    const py = ((e.clientY - rect.top) / rect.height) * 2 - 1;
    el.style.setProperty("--px", px.toFixed(2));
    el.style.setProperty("--py", py.toFixed(2));
  };

  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    press.current = { x: e.clientX, y: e.clientY };
    setPressed(true);
  };
  const onPointerMove = (e: PointerEvent) => {
    if (dragging) endDrag();
    track(e);
    const start = press.current;
    if (start && Math.hypot(e.clientX - start.x, e.clientY - start.y) > DRAG_THRESHOLD) {
      press.current = null;
      setPressed(false);
      setDragging(true);
      settle.current = window.setTimeout(endDrag, DRAG_SETTLE_MS);
      currentWindow.startDragging();
    }
  };
  const onPointerUp = () => {
    if (press.current) summon();
    press.current = null;
    setPressed(false);
  };
  const onPointerLeave = () => {
    press.current = null;
    setPressed(false);
    stage.current?.style.setProperty("--px", "0");
    stage.current?.style.setProperty("--py", "0");
  };

  return (
    <main
      ref={stage}
      className={styles.stage}
      data-state={state}
      data-pressed={pressed || undefined}
      data-dragging={dragging || undefined}
    >
      <button
        type="button"
        className={styles.companion}
        aria-label={t("companion.open", { state: stateLabel(t, state) })}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={onPointerLeave}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") summon();
        }}
      >
        <Renderer state={state} size={size} intensity={intensity} pulseKey={revision} />
      </button>
    </main>
  );
}
