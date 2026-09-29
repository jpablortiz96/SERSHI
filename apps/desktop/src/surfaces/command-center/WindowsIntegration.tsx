import type {
  ApplicationCatalogInfo,
  FeatureStatus,
  IntegrationStatus,
  ShortcutProblem,
  ShortcutStatus,
} from "@sershi/contracts";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import { useI18n } from "../../i18n";
import { applicationName } from "../../i18n/domain";
import { desktopRuntime, sershi } from "../../ipc";
import { recordKey, useShortcut } from "../../state/shortcut";
import styles from "./Page.module.css";
import { Pill, Row, Section } from "./SettingsParts";

const TONE: Record<FeatureStatus, "success" | "warning" | "neutral"> = {
  active: "success",
  unavailable: "warning",
  planned: "neutral",
};

function useWindowsIntegration() {
  const [integration, setIntegration] = useState<IntegrationStatus | null>(null);
  const [catalog, setCatalog] = useState<ApplicationCatalogInfo | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    if (!desktopRuntime) return;
    sershi.getIntegrationStatus().then(setIntegration, () => undefined);
    sershi.getApplicationCatalog().then(setCatalog, () => undefined);
  }, []);

  const refresh = () => {
    setRefreshing(true);
    sershi
      .refreshApplicationCatalog()
      .then(setCatalog, () => {
        void sershi.getApplicationCatalog().then(setCatalog, () => undefined);
      })
      .finally(() => {
        setRefreshing(false);
      });
  };

  return { integration, catalog, refreshing, refresh };
}

/** Settings → Windows integration: honest status of every OS integration. */
export function WindowsIntegration() {
  const { t, format } = useI18n();
  const { integration, catalog, refreshing, refresh } = useWindowsIntegration();

  if (!desktopRuntime) {
    return (
      <Section title={t("settings.sections.windows")}>
        <p className={styles.empty}>{t("settings.desktopOnly")}</p>
      </Section>
    );
  }

  const status = catalog?.status;
  const unsupported = status?.state === "unsupported";
  const catalogDetail = !status
    ? null
    : status.state === "ready" && status.refreshedAtMs != null
      ? t("settings.windows.catalogScanned", {
          time: format.time(status.refreshedAtMs),
          duration: format.milliseconds(status.scanDurationMs ?? 0),
        })
      : status.state === "failed"
        ? t("settings.windows.catalogFailed")
        : status.state === "notScanned"
          ? t("settings.windows.catalogNotScanned")
          : null;

  return (
    <Section title={t("settings.sections.windows")}>
      <Row
        label={t("settings.windows.appControl")}
        detail={t("settings.windows.appControlDetail")}
        value={
          <Pill tone={unsupported ? "neutral" : "success"}>
            {unsupported
              ? t("settings.windows.catalogUnsupported")
              : t("settings.featureStatus.active")}
          </Pill>
        }
      />
      {integration && (
        <>
          <Row
            label={t("settings.windows.tray")}
            detail={t("settings.windows.trayDetail")}
            value={
              <Pill tone={TONE[integration.tray]}>
                {t(`settings.featureStatus.${integration.tray}`)}
              </Pill>
            }
          />
          <ShortcutSetting initial={integration.shortcut} />
          <Row
            label={t("settings.windows.startup")}
            value={
              <Pill tone={TONE[integration.startWithWindows]}>
                {t(`settings.featureStatus.${integration.startWithWindows}`)}
              </Pill>
            }
          />
        </>
      )}
      {status && !unsupported && (
        <Row
          label={t("settings.windows.catalog")}
          detail={catalogDetail}
          value={
            <span className={styles.pills}>
              {status.state === "ready" && (
                <span>
                  {t("settings.windows.catalogCount", { count: format.integer(status.count) })}
                </span>
              )}
              <button
                type="button"
                className={styles.secondary}
                disabled={refreshing}
                onClick={refresh}
              >
                {refreshing ? t("settings.windows.refreshing") : t("settings.windows.refresh")}
              </button>
            </span>
          }
        />
      )}
    </Section>
  );
}

/** Developer Mode: what discovery found (names and sources, never paths). */
export function CatalogInspector() {
  const { t } = useI18n();
  const { catalog } = useWindowsIntegration();
  const apps = catalog?.applications ?? [];

  return (
    <Section title={t("settings.developer.catalog")}>
      {apps.length > 0 ? (
        <ul className={styles.catalog}>
          {apps.map((app) => (
            <li key={app.id}>
              <span>{applicationName(t, app)}</span>
              <span className="t-caption">{t(`settings.sources.${app.source}`)}</span>
            </li>
          ))}
        </ul>
      ) : (
        <p className={styles.empty}>
          {!desktopRuntime
            ? t("settings.desktopOnly")
            : catalog?.status.state === "unsupported"
              ? t("settings.windows.catalogUnsupported")
              : t("settings.windows.catalogNotScanned")}
        </p>
      )}
    </Section>
  );
}

function Keys({ accelerator }: { accelerator: string }) {
  return (
    <span className={styles.keys}>
      {accelerator.split("+").map((key) => (
        <kbd key={key}>{key}</kbd>
      ))}
    </span>
  );
}

/**
 * Global shortcut: shows the current combination and records a new one.
 * The recorder is a focused button: the next key combination is captured,
 * Escape cancels, Tab still moves focus. Windows decides whether it is
 * available; a refused shortcut never replaces the working one.
 */
function ShortcutSetting({ initial }: { initial: ShortcutStatus }) {
  const { t } = useI18n();
  const status = useShortcut((s) => s.status) ?? initial;
  const lastChange = useShortcut((s) => s.lastChange);
  const applying = useShortcut((s) => s.applying);
  const change = useShortcut((s) => s.change);
  const clearFeedback = useShortcut((s) => s.clearFeedback);
  const [recording, setRecording] = useState(false);
  const [held, setHeld] = useState<string[]>([]);
  const [problem, setProblem] = useState<ShortcutProblem | null>(null);
  const recorder = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (recording) recorder.current?.focus();
  }, [recording]);

  const stop = () => {
    setRecording(false);
    setHeld([]);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording || e.key === "Tab") return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey) {
      stop();
      return;
    }
    const recorded = recordKey(e.nativeEvent);
    if (recorded.kind === "modifiers") {
      setHeld(recorded.held);
      setProblem(null);
    } else if (recorded.kind === "invalid") {
      setProblem(recorded.problem);
    } else {
      stop();
      setProblem(null);
      void change(recorded.accelerator);
    }
  };

  const feedback = problem
    ? { tone: "warning" as const, text: t(`settings.windows.shortcutProblems.${problem}`) }
    : lastChange?.result === "unavailable"
      ? {
          tone: "warning" as const,
          title: t("settings.windows.shortcutUnavailableTitle"),
          text: t("settings.windows.shortcutUnavailableBody"),
        }
      : lastChange?.result === "invalid" && lastChange.problem
        ? {
            tone: "warning" as const,
            text: t(`settings.windows.shortcutProblems.${lastChange.problem}`),
          }
        : lastChange?.result === "registered" || lastChange?.result === "unchanged"
          ? {
              tone: "success" as const,
              text: t("settings.windows.shortcutSaved", {
                keys: lastChange.shortcut.accelerator,
              }),
            }
          : null;

  return (
    <div className={styles.shortcut}>
      <Row
        label={t("settings.windows.shortcut")}
        detail={
          status.status === "unavailable"
            ? t("settings.windows.shortcutUnavailable")
            : t("settings.windows.shortcutDetail")
        }
        value={
          <span className={styles.pills}>
            <Keys accelerator={status.accelerator} />
            <Pill tone={TONE[status.status]}>{t(`settings.featureStatus.${status.status}`)}</Pill>
            {recording ? (
              <button
                ref={recorder}
                type="button"
                className={styles.recorder}
                aria-describedby="shortcut-recording-hint"
                onKeyDown={onKeyDown}
                onBlur={stop}
                onClick={stop}
              >
                {held.length > 0 ? <Keys accelerator={`${held.join("+")}+…`} /> : null}
                <span>{t("settings.windows.shortcutRecording")}</span>
              </button>
            ) : (
              <button
                type="button"
                className={styles.secondary}
                disabled={applying}
                onClick={() => {
                  clearFeedback();
                  setProblem(null);
                  setRecording(true);
                }}
              >
                {t("settings.windows.shortcutChange")}
              </button>
            )}
          </span>
        }
      />
      <div className={styles.shortcutFeedback} role="status" aria-live="polite">
        {recording && (
          <p id="shortcut-recording-hint" className={styles.rowDetail}>
            {t("settings.windows.shortcutRecordingHint")}
          </p>
        )}
        {recording && problem && (
          <p data-tone="warning">{t(`settings.windows.shortcutProblems.${problem}`)}</p>
        )}
        {!recording && feedback && (
          <p data-tone={feedback.tone}>
            {"title" in feedback && feedback.title && <strong>{feedback.title}. </strong>}
            {feedback.text}
          </p>
        )}
        {!recording && lastChange?.altgrWarning && lastChange.result !== "invalid" && (
          <p data-tone="warning">{t("settings.windows.shortcutAltGr")}</p>
        )}
      </div>
    </div>
  );
}
