import type { CSSProperties } from "react";

import { formatGigabytes, formatPercent, formatUptime } from "../../lib/format";
import { useTelemetry } from "../../state/telemetry";
import rail from "../shell/Rail.module.css";
import { Sparkline } from "./Sparkline";
import styles from "./SystemRail.module.css";

/**
 * Supporting telemetry. Deliberately quiet: SERSHI is not a system monitor,
 * so numbers are small, neutral and never compete with the core.
 */
export function SystemRail() {
  const { snapshot, cpuHistory, error } = useTelemetry();
  const cpu = snapshot?.cpu.usagePercent ?? null;
  const memory = snapshot?.memory;
  const memoryRatio = memory && memory.totalBytes > 0 ? memory.usedBytes / memory.totalBytes : 0;

  return (
    <aside className={rail.rail} aria-labelledby="system-heading">
      <h2 id="system-heading" className="t-label">
        This computer
      </h2>

      {!snapshot ? (
        <p className={rail.empty}>{error ?? "Reading system…"}</p>
      ) : (
        <>
          <div className={styles.os}>
            <p className={styles.osName}>{snapshot.os.name}</p>
            <p className="t-caption">
              {snapshot.os.arch} · up {formatUptime(snapshot.uptimeSecs)}
            </p>
          </div>

          <section className={styles.metric} aria-label="Processor">
            <div className={styles.metricHead}>
              <span className="t-label">Processor</span>
              <span className={styles.value}>
                {cpu === null ? "—" : formatPercent(cpu)}
                <small>%</small>
              </span>
            </div>
            <Sparkline values={cpuHistory} />
            <p className="t-caption" title={snapshot.cpu.brand}>
              {snapshot.cpu.logicalCores} threads · {snapshot.cpu.brand}
            </p>
          </section>

          {memory && (
            <section className={styles.metric} aria-label="Memory">
              <div className={styles.metricHead}>
                <span className="t-label">Memory</span>
                <span className={styles.value}>
                  {formatGigabytes(memory.usedBytes)}
                  <small> / {formatGigabytes(memory.totalBytes)} GB</small>
                </span>
              </div>
              <div
                className={styles.meter}
                role="meter"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(memoryRatio * 100)}
                aria-label="Memory in use"
              >
                <i style={{ "--ratio": memoryRatio } as CSSProperties} />
              </div>
              <p className="t-caption">{formatPercent(memoryRatio * 100)}% in use</p>
            </section>
          )}
        </>
      )}
    </aside>
  );
}
