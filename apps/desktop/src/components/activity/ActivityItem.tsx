import type { ActivityEntry, ActivityKind } from "@sershi/contracts";

import { formatClock } from "../../lib/format";
import styles from "./Activity.module.css";

export const KIND_TONE: Record<
  ActivityKind,
  "neutral" | "success" | "warning" | "error" | "signal"
> = {
  systemReady: "signal",
  commandReceived: "neutral",
  toolRequested: "neutral",
  toolCompleted: "success",
  toolFailed: "error",
  toolDenied: "error",
  confirmationRequired: "warning",
  capabilityUnavailable: "warning",
};

export function ActivityItem({ entry }: { entry: ActivityEntry }) {
  return (
    <li className={styles.item} data-tone={KIND_TONE[entry.kind]}>
      <i className={styles.dot} aria-hidden="true" />
      <p className={styles.summary}>{entry.summary}</p>
      <p className={styles.meta}>
        <time className="t-mono" dateTime={new Date(entry.atMs).toISOString()}>
          {formatClock(entry.atMs)}
        </time>
        {entry.durationMs != null && <span className="t-mono">{entry.durationMs} ms</span>}
      </p>
    </li>
  );
}
