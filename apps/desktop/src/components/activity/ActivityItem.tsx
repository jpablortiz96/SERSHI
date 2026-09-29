import type { ActivityEntry, ActivityKind } from "@sershi/contracts";

import { useI18n } from "../../i18n";
import { describeActivity } from "../../i18n/domain";
import type { StateGlyph } from "../../visual/stateVisuals";
import { Glyph } from "../core/StateGlyph";
import styles from "./Activity.module.css";

export type Tone = "neutral" | "success" | "warning" | "error" | "signal";

/** Shape per tone, so an entry's meaning never depends on colour alone. */
export const TONE_GLYPH: Record<Tone, StateGlyph> = {
  neutral: "hollow",
  success: "check",
  warning: "triangle",
  error: "cross",
  signal: "dot",
};

export const KIND_TONE: Record<ActivityKind, Tone> = {
  systemReady: "signal",
  commandReceived: "neutral",
  toolRequested: "neutral",
  toolCompleted: "success",
  toolFailed: "error",
  toolDenied: "error",
  toolDeclined: "warning",
  confirmationRequired: "warning",
  confirmationApproved: "neutral",
  confirmationCancelled: "neutral",
  confirmationExpired: "neutral",
  capabilityUnavailable: "warning",
  // The microphone opening and closing is a system fact worth seeing.
  microphoneOn: "signal",
  microphoneOff: "signal",
};

export function ActivityItem({ entry }: { entry: ActivityEntry }) {
  const { t, format } = useI18n();
  return (
    <li className={styles.item} data-tone={KIND_TONE[entry.kind]}>
      <Glyph shape={TONE_GLYPH[KIND_TONE[entry.kind]]} className={styles.dot} />
      <p className={styles.summary}>{describeActivity(t, entry)}</p>
      <p className={styles.meta}>
        <time className="t-mono" dateTime={new Date(entry.atMs).toISOString()}>
          {format.time(entry.atMs)}
        </time>
        {entry.durationMs != null && (
          <span className="t-mono">{format.milliseconds(entry.durationMs)}</span>
        )}
      </p>
    </li>
  );
}
