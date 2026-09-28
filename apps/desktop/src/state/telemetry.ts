import type { SystemSnapshot } from "@sershi/contracts";
import { useEffect, useState } from "react";

import { desktopRuntime, sershi } from "../ipc";

export interface Telemetry {
  snapshot: SystemSnapshot | null;
  /** Recent CPU readings (0–100), oldest first. */
  cpuHistory: number[];
  /** Why no snapshot is available (rendered as localized copy). */
  error: "browserPreview" | "unavailable" | null;
}

const HISTORY = 36;

/**
 * Polls system telemetry while the window is visible. Polling stops when the
 * window is hidden so an idle SERSHI costs (almost) nothing.
 */
export function useTelemetry(intervalMs = 2000): Telemetry {
  const [telemetry, setTelemetry] = useState<Telemetry>({
    snapshot: null,
    cpuHistory: [],
    error: desktopRuntime ? null : "browserPreview",
  });

  useEffect(() => {
    if (!desktopRuntime) return;
    let timer: number | undefined;
    let cancelled = false;

    const tick = async () => {
      try {
        const snapshot = await sershi.getSystemSnapshot();
        if (cancelled) return;
        setTelemetry((t) => ({
          snapshot,
          error: null,
          cpuHistory:
            snapshot.cpu.usagePercent === null
              ? t.cpuHistory
              : [...t.cpuHistory, snapshot.cpu.usagePercent].slice(-HISTORY),
        }));
      } catch {
        if (!cancelled) setTelemetry((t) => ({ ...t, error: "unavailable" }));
      }
    };

    const schedule = () => {
      window.clearInterval(timer);
      if (document.visibilityState === "visible") {
        void tick();
        timer = window.setInterval(() => void tick(), intervalMs);
      }
    };

    schedule();
    document.addEventListener("visibilitychange", schedule);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", schedule);
    };
  }, [intervalMs]);

  return telemetry;
}
