import { useEffect } from "react";

import { useI18n } from "../../i18n";
import { applicationName } from "../../i18n/domain";
import { desktopRuntime } from "../../ipc";
import { useUnderstanding } from "../../state/understanding";
import styles from "./Page.module.css";
import { Choices, Pill, Row, Section } from "./SettingsParts";

/**
 * Natural command understanding: the optional local language model. It
 * only interprets what the user says; SERSHI's rules still decide, and
 * nothing here can run, approve or grant anything. Without it, SERSHI still
 * understands common commands, similar names and follow-up answers.
 */
export function UnderstandingSettings() {
  const { t, format } = useI18n();
  const status = useUnderstanding((s) => s.status);
  const enabled = useUnderstanding((s) => s.enabled);
  const error = useUnderstanding((s) => s.error);
  const refresh = useUnderstanding((s) => s.refresh);
  const setEnabled = useUnderstanding((s) => s.setEnabled);
  const download = useUnderstanding((s) => s.download);
  const cancelDownload = useUnderstanding((s) => s.cancelDownload);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!desktopRuntime || !status) {
    return (
      <Section title={t("settings.sections.understanding")}>
        <p className={styles.lede}>{t("settings.understanding.lede")}</p>
        <p className={styles.empty}>{t("settings.desktopOnly")}</p>
      </Section>
    );
  }

  const model = status.model;
  const percent =
    model.state.kind === "downloading"
      ? Math.min(100, (model.state.receivedBytes / model.sizeBytes) * 100)
      : null;
  const installed = model.state.kind === "installed";

  let action = null;
  if (percent !== null) {
    action = (
      <button type="button" className={styles.secondary} onClick={cancelDownload}>
        {t("voice.model.cancel")}
      </button>
    );
  } else if (!installed && status.available) {
    action = (
      <button type="button" className={styles.secondary} onClick={download}>
        {model.state.kind === "corrupt"
          ? t("settings.understanding.redownload")
          : t("settings.understanding.download", { size: format.megabytes(model.sizeBytes) })}
      </button>
    );
  }

  return (
    <Section title={t("settings.sections.understanding")}>
      <p className={styles.lede}>{t("settings.understanding.lede")}</p>
      {status.available ? (
        <>
          <div className={styles.rowStacked}>
            <div>
              <p className={styles.rowLabel}>{t("settings.understanding.model")}</p>
              <p className={styles.rowDetail}>
                {t("settings.understanding.modelMeta", {
                  name: model.name,
                  quantization: model.quantization,
                  size: format.megabytes(model.sizeBytes),
                  license: model.license,
                })}
              </p>
              <p className={styles.rowDetail}>
                {model.vramMb > 0
                  ? t("settings.understanding.memoryGpu", {
                      ram: format.megabytes(model.ramMb * 1_000_000),
                      vram: format.megabytes(model.vramMb * 1_000_000),
                    })
                  : t("settings.understanding.memoryCpu", {
                      ram: format.megabytes(model.ramMb * 1_000_000),
                    })}
              </p>
              {percent !== null && (
                <progress
                  className={styles.progress}
                  max={100}
                  value={percent}
                  aria-label={t("voice.model.downloading", { percent: format.percent(percent) })}
                />
              )}
              {error && (
                <p className={styles.rowDetail} role="alert">
                  <Pill tone="warning">{t(`voice.model.error.${error}`)}</Pill>
                </p>
              )}
            </div>
            <div className={styles.modelActions}>
              {installed && <Pill tone="success">{t("settings.understanding.installed")}</Pill>}
              {model.state.kind === "notInstalled" && (
                <Pill tone="neutral">{t("settings.understanding.notInstalled")}</Pill>
              )}
              {model.state.kind === "corrupt" && (
                <Pill tone="warning">{t("settings.understanding.corrupt")}</Pill>
              )}
              {action}
            </div>
          </div>
          {installed && (
            <>
              <Choices<"on" | "off">
                name="natural-understanding"
                label={t("settings.understanding.enabled")}
                detail={t("settings.understanding.enabledDetail")}
                options={["on", "off"]}
                value={enabled ? "on" : "off"}
                onChange={(value) => {
                  setEnabled(value === "on");
                }}
                optionLabel={(value) => t(`settings.voice.options.${value}`)}
              />
              <Row
                label={t("settings.understanding.runtime")}
                detail={t("settings.understanding.runtimeDetail")}
                value={
                  status.running
                    ? t("settings.understanding.loaded", {
                        backend:
                          status.backend === "cpu"
                            ? t("settings.understanding.backendCpu")
                            : (status.device ?? t("settings.understanding.backendGpu")),
                      })
                    : t("settings.understanding.notLoaded")
                }
              />
            </>
          )}
        </>
      ) : (
        <p className={styles.empty}>{t("settings.understanding.unavailable")}</p>
      )}
      <p className={styles.footnote}>{t("settings.understanding.footnote")}</p>
    </Section>
  );
}

/**
 * Developer Mode: how the last request was understood. Transient (memory
 * only) and never stored; the model's prompt is never shown.
 */
export function UnderstandingDiagnostics() {
  const { t, format } = useI18n();
  const last = useUnderstanding((s) => s.last);
  if (!last) {
    return (
      <Section title={t("settings.developer.understanding.title")}>
        <p className={styles.empty}>{t("settings.developer.understanding.empty")}</p>
      </Section>
    );
  }
  const { trace } = last;
  return (
    <Section title={t("settings.developer.understanding.title")}>
      <p className={styles.footnote}>{t("settings.developer.understanding.footnote")}</p>
      {last.raw !== null && (
        <Row label={t("settings.developer.understanding.raw")} value={last.raw} />
      )}
      <Row
        label={t("settings.developer.understanding.normalized")}
        value={<span className="t-mono">{trace.normalized}</span>}
      />
      <Row
        label={t("settings.developer.understanding.tier")}
        value={t(`settings.developer.understanding.tiers.${trace.tier}`)}
      />
      <Row
        label={t("settings.developer.understanding.result")}
        value={<span className="t-mono">{last.status}</span>}
      />
      <Row
        label={t("settings.developer.understanding.confidence")}
        value={format.percent(trace.confidence * 100)}
      />
      <Row
        label={t("settings.developer.understanding.semantic")}
        value={t(`settings.developer.understanding.semanticUse.${trace.semantic}`)}
      />
      <Row
        label={t("settings.developer.understanding.time")}
        value={
          <span className="t-mono">
            {format.milliseconds(Math.round(trace.understandingMs))}
            {trace.semanticMs != null &&
              ` · ${t("settings.developer.understanding.modelTime", {
                ms: format.milliseconds(trace.semanticMs),
              })}`}
          </span>
        }
      />
      {trace.candidates.length > 0 && (
        <Row
          label={t("settings.developer.understanding.candidates")}
          value={
            <span className="t-mono">
              {trace.candidates
                .map((c) => `${applicationName(t, c.application)} ${c.score.toFixed(2)}`)
                .join(" · ")}
            </span>
          }
        />
      )}
    </Section>
  );
}
