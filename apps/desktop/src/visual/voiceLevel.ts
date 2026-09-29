/**
 * Real audio reactivity, restrained. The core sends a bounded 0–1 level
 * (microphone while Listening, SERSHI's own voice while Speaking) at most
 * ~25 times per second; raw audio never reaches the UI. This writes it to
 * `--voice-level` on `<html>` once per frame, where the Core's listening
 * and speaking patterns read it. With reduced motion the stylesheet ignores
 * it: state stays readable from colour, shape and label.
 */
import { sershi } from "../ipc";

export function connectVoiceLevel(): () => void {
  const root = document.documentElement;
  let pending: number | null = null;
  let frame = 0;
  const write = () => {
    frame = 0;
    if (pending === null) return;
    root.style.setProperty("--voice-level", pending.toFixed(3));
    pending = null;
  };
  const stop = sershi.onVoiceLevel(({ level }) => {
    pending = Math.min(1, Math.max(0, level));
    if (!frame) frame = requestAnimationFrame(write);
  });
  return () => {
    stop();
    if (frame) cancelAnimationFrame(frame);
    root.style.removeProperty("--voice-level");
  };
}
