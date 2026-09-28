import type { CSSProperties } from "react";

import { useI18n } from "../../i18n";
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
  const { t, format } = useI18n();
  const cpu = snapshot?.cpu.usagePercent ?? null;
  const cpuParts = cpu === null ? null : format.percentParts(cpu);
  const memory = snapshot?.memory;
  const memoryRatio = memory && memory.totalBytes > 0 ? memory.usedBytes / memory.totalBytes : 0;

  const unavailable =
    error === "browserPreview"
      ? t("system.browserPreview")
      : error === "unavailable"
        ? t("system.unavailable")
        : t("system.reading");

  return (
    <aside className={rail.rail} aria-labelledby="system-heading">
      <h2 id="system-heading" className="t-label">
        {t("system.heading")}
      </h2>

      {!snapshot ? (
        <p className={rail.empty}>{unavailable}</p>
      ) : (
        <>
          <div className={styles.os}>
            <p className={styles.osName}>{snapshot.os.name}</p>
            <p className="t-caption">
              {t("system.osLine", {
                arch: snapshot.os.arch,
                uptime: format.uptime(snapshot.uptimeSecs),
              })}
            </p>
          </div>

          <section className={styles.metric} aria-label={t("system.processor")}>
            <div className={styles.metricHead}>
              <span className="t-label">{t("system.processor")}</span>
              <span className={styles.value}>
                {cpuParts ? cpuParts.number : "—"}
                {cpuParts && <small>{cpuParts.sign}</small>}
              </span>
            </div>
            <Sparkline values={cpuHistory} />
            <p className="t-caption" title={snapshot.cpu.brand}>
              {t("system.threads", {
                count: format.integer(snapshot.cpu.logicalCores),
                cpu: snapshot.cpu.brand,
              })}
            </p>
          </section>

          {memory && (
            <section className={styles.metric} aria-label={t("system.memory")}>
              <div className={styles.metricHead}>
                <span className="t-label">{t("system.memory")}</span>
                <span className={styles.value}>
                  {format.gigabytes(memory.usedBytes)}
                  <small>
                    {" "}
                    {t("system.memoryTotal", { total: format.gigabytes(memory.totalBytes) })}
                  </small>
                </span>
              </div>
              <div
                className={styles.meter}
                role="meter"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(memoryRatio * 100)}
                aria-valuetext={format.percent(memoryRatio * 100)}
                aria-label={t("system.memoryMeter")}
              >
                <i style={{ "--ratio": memoryRatio } as CSSProperties} />
              </div>
              <p className="t-caption">
                {t("system.memoryInUse", { percent: format.percent(memoryRatio * 100) })}
              </p>
            </section>
          )}
        </>
      )}
    </aside>
  );
}
