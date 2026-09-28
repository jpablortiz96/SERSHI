import type { ActivityEntry, ActivityKind } from "@sershi/contracts";

import { useI18n } from "../../i18n";
import { describeActivity } from "../../i18n/domain";
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
  const { t, format } = useI18n();
  return (
    <li className={styles.item} data-tone={KIND_TONE[entry.kind]}>
      <i className={styles.dot} aria-hidden="true" />
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
