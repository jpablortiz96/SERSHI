import type { ModelInfo } from "@sershi/contracts";

import { useI18n } from "../../i18n";
import { modelName } from "../../i18n/domain";
import { useVoice } from "../../state/voice";
import styles from "./VoicePanel.module.css";

/** Download progress (0–100), or null when not downloading. */
export function downloadPercent(model: ModelInfo | undefined): number | null {
  if (model?.state.kind !== "downloading") return null;
  return Math.min(100, (model.state.receivedBytes / model.sizeBytes) * 100);
}

/**
 * Explains what voice needs or what went wrong, next to the command bar.
 * The model download is offered here — never started silently.
 */
export function VoicePanel() {
  const { t, format } = useI18n();
  const notice = useVoice((s) => s.notice);
  const status = useVoice((s) => s.status);
  const download = useVoice((s) => s.download);
  const cancelDownload = useVoice((s) => s.cancelDownload);
  const dismiss = useVoice((s) => s.dismissNotice);

  const model = status?.models.find((m) => m.id === status.model);
  const percent = downloadPercent(model);

  if (percent !== null && model) {
    return (
      <div className={styles.panel} role="status">
        <p className={styles.title}>{modelName(t, model.id)}</p>
        <p>{t("voice.model.downloading", { percent: format.percent(percent) })}</p>
        <progress className={styles.progress} max={100} value={percent} />
        <div className={styles.actions}>
          <button type="button" onClick={cancelDownload}>
            {t("voice.model.cancel")}
          </button>
        </div>
      </div>
    );
  }

  if (!notice) return null;

  if (notice.kind === "modelRequired" && model) {
    return (
      <div className={styles.panel} role="alert">
        <p className={styles.title}>{t("voice.model.title")}</p>
        <p>{t("voice.model.body")}</p>
        <dl className={styles.facts}>
          <dt>{t("voice.model.name")}</dt>
          <dd>{modelName(t, model.id)}</dd>
          <dt>{t("voice.model.download")}</dt>
          <dd>{format.megabytes(model.sizeBytes)}</dd>
          <dt>{t("voice.model.storage")}</dt>
          <dd>{format.megabytes(model.sizeBytes)}</dd>
          <dt>{t("voice.model.memory")}</dt>
          <dd>{format.megabytes(model.memoryMb * 1_000_000)}</dd>
        </dl>
        <div className={styles.actions}>
          <button
            type="button"
            className={styles.primary}
            onClick={() => {
              download(model.id);
            }}
          >
            {t("voice.model.action")}
          </button>
          <button type="button" onClick={dismiss}>
            {t("voice.model.later")}
          </button>
        </div>
      </div>
    );
  }

  const message = (() => {
    switch (notice.kind) {
      case "failure":
        return t(`voice.failure.${notice.reason}`);
      case "fallback":
        return t("voice.fallback");
      case "modelError":
        return t(`voice.model.error.${notice.error}`);
      case "modelReady":
        return t("voice.model.ready");
      case "modelRequired":
        return t("voice.failure.modelMissing");
    }
  })();

  return (
    <div
      className={styles.panel}
      role={notice.kind === "failure" ? "alert" : "status"}
      data-tone={notice.kind === "modelReady" ? "success" : "warning"}
    >
      <p>{message}</p>
      <div className={styles.actions}>
        <button type="button" onClick={dismiss}>
          {t("voice.dismiss")}
        </button>
      </div>
    </div>
  );
}
