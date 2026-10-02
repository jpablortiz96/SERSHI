import { useEffect } from "react";

import { useI18n } from "../../i18n";
import { applicationName, stepLabel } from "../../i18n/domain";
import { desktopRuntime } from "../../ipc";
import { MODEL_ROLES, useUnderstanding, type ModelRole } from "../../state/understanding";
import { useVoice } from "../../state/voice";
import styles from "./Page.module.css";
import { Choices, Pill, Row, Section } from "./SettingsParts";

/**
 * Intelligence: the two optional local language models. The Agent Brain
 * (conversation, follow-ups, short plans) and the semantic router (short,
 * imperfect commands). They only interpret and propose; SERSHI's rules
 * decide, sensitive actions still need the confirmation window, and nothing
 * here can run, approve or grant anything. Without them, SERSHI still
 * understands common commands, similar names, references and follow-ups.
 */
export function UnderstandingSettings() {
  const { t } = useI18n();
  const refresh = useUnderstanding((s) => s.refresh);

  useEffect(() => {
    for (const role of MODEL_ROLES) void refresh(role);
  }, [refresh]);

  return (
    <Section title={t("settings.sections.understanding")}>
      <p className={styles.lede}>{t("settings.understanding.lede")}</p>
      {desktopRuntime ? (
        MODEL_ROLES.map((role) => <ModelCard key={role} role={role} />)
      ) : (
        <p className={styles.empty}>{t("settings.desktopOnly")}</p>
      )}
      <p className={styles.footnote}>{t("settings.understanding.footnote")}</p>
    </Section>
  );
}

function ModelCard({ role }: { role: ModelRole }) {
  const { t, format } = useI18n();
  const { status, enabled, error } = useUnderstanding((s) => s.models[role]);
  const setEnabled = useUnderstanding((s) => s.setEnabled);
  const download = useUnderstanding((s) => s.download);
  const cancelDownload = useUnderstanding((s) => s.cancelDownload);
  const copy = (key: string) =>
    t(`settings.understanding.${role}.${key}` as "settings.understanding.brain.title");

  if (!status) return null;
  const model = status.model;
  const percent =
    model.state.kind === "downloading"
      ? Math.min(100, (model.state.receivedBytes / model.sizeBytes) * 100)
      : null;
  const installed = model.state.kind === "installed";

  let action = null;
  if (percent !== null) {
    action = (
      <button
        type="button"
        className={styles.secondary}
        onClick={() => {
          cancelDownload(role);
        }}
      >
        {t("voice.model.cancel")}
      </button>
    );
  } else if (!installed && status.available) {
    action = (
      <button
        type="button"
        className={styles.secondary}
        disabled={!status.freeSpaceOk}
        onClick={() => {
          download(role);
        }}
      >
        {model.state.kind === "corrupt"
          ? t("settings.understanding.redownload")
          : t("settings.understanding.download", { size: format.megabytes(model.sizeBytes) })}
      </button>
    );
  }

  return (
    <div className={styles.rowStacked} data-model={role}>
      <div>
        <p className={styles.rowLabel}>{copy("title")}</p>
        <p className={styles.rowDetail}>{copy("detail")}</p>
        {status.available ? (
          <>
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
            {role === "brain" && model.vramMb === 0 && (
              <p className={styles.rowDetail} role="note">
                <Pill tone="neutral">{t("settings.understanding.brainCpuNote")}</Pill>
              </p>
            )}
            {!installed && !status.freeSpaceOk && (
              <p className={styles.rowDetail} role="alert">
                <Pill tone="warning">{t("settings.understanding.noSpace")}</Pill>
              </p>
            )}
            {status.standby && (
              <p className={styles.rowDetail} role="note">
                <Pill tone="neutral">{t("settings.understanding.standby")}</Pill>
              </p>
            )}
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
            {installed && (
              <>
                <Choices<"on" | "off">
                  name={`local-model-${role}`}
                  label={copy("enabled")}
                  detail={copy("enabledDetail")}
                  options={["on", "off"]}
                  value={enabled ? "on" : "off"}
                  onChange={(value) => {
                    setEnabled(role, value === "on");
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
  );
}

/**
 * Developer Mode: how the last request was understood and routed.
 * Transient (memory only), never stored; the model's prompt and reasoning
 * are never shown.
 */
export function UnderstandingDiagnostics() {
  const { t, format } = useI18n();
  const last = useUnderstanding((s) => s.last);
  const voiceSession = useVoice((s) => s.session ?? s.lastSession);
  if (!last) {
    return (
      <Section title={t("settings.developer.understanding.title")}>
        <p className={styles.empty}>{t("settings.developer.understanding.empty")}</p>
      </Section>
    );
  }
  const { trace, brain } = last;
  const session = last.session;
  return (
    <Section title={t("settings.developer.understanding.title")}>
      <p className={styles.footnote}>{t("settings.developer.understanding.footnote")}</p>
      {last.raw !== null && (
        <Row label={t("settings.developer.understanding.raw")} value={last.raw} />
      )}
      {brain && (
        <>
          <Row
            label={t("settings.developer.understanding.route")}
            value={t(`settings.developer.understanding.routes.${brain.route}`)}
          />
          {brain.brain && (
            <Row
              label={t("settings.developer.understanding.brain")}
              value={t(`settings.developer.understanding.brainUse.${brain.brain}`)}
            />
          )}
          {brain.modelMs != null && (
            <Row
              label={t("settings.developer.understanding.brainTime")}
              value={<span className="t-mono">{format.milliseconds(brain.modelMs)}</span>}
            />
          )}
          {brain.steps > 0 && (
            <Row
              label={t("settings.developer.understanding.planSteps")}
              value={<span className="t-mono">{format.integer(brain.steps)}</span>}
            />
          )}
          <Row
            label={t("settings.developer.understanding.promptVersion")}
            value={<span className="t-mono">{brain.promptVersion}</span>}
          />
        </>
      )}
      <Row
        label={t("settings.developer.understanding.result")}
        value={<span className="t-mono">{last.status}</span>}
      />
      {session && (
        <>
          <Row
            label={t("settings.developer.understanding.session")}
            value={<span className="t-mono">#{session.session}</span>}
          />
          <Row
            label={t("settings.developer.understanding.modality")}
            value={t(`settings.developer.understanding.modalities.${session.modality}`)}
          />
          <Row
            label={t("settings.developer.understanding.entities")}
            value={<span className="t-mono">{format.integer(session.activeEntities)}</span>}
          />
          <Row
            label={t("settings.developer.understanding.ledger")}
            value={<span className="t-mono">{format.integer(session.ledgerEntries)}</span>}
          />
          {session.lastAction && (
            <Row
              label={t("settings.developer.understanding.lastAction")}
              value={stepLabel(t, session.lastAction)}
            />
          )}
          {session.reference && (
            <Row
              label={t("settings.developer.understanding.reference")}
              value={t(`settings.developer.understanding.references.${session.reference}`)}
            />
          )}
          {session.authorization && (
            <Row
              label={t("settings.developer.understanding.authorization")}
              value={t(`settings.developer.understanding.authorizations.${session.authorization}`)}
            />
          )}
        </>
      )}
      {voiceSession && (
        <>
          <Row
            label={t("settings.developer.understanding.voiceSession")}
            value={
              voiceSession.phase
                ? t(`session.phase.${voiceSession.phase}`)
                : t(`session.ended.${voiceSession.ended ?? "stopped"}`)
            }
          />
          <Row
            label={t("settings.developer.understanding.bargeIns")}
            value={<span className="t-mono">{format.integer(voiceSession.bargeIns)}</span>}
          />
        </>
      )}
      {trace && (
        <>
          <Row
            label={t("settings.developer.understanding.normalized")}
            value={<span className="t-mono">{trace.normalized}</span>}
          />
          <Row
            label={t("settings.developer.understanding.tier")}
            value={t(`settings.developer.understanding.tiers.${trace.tier}`)}
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
        </>
      )}
    </Section>
  );
}
