import type {
  ApplicationCatalogInfo,
  ConfirmationRequest,
  FeatureStatus,
  IntegrationStatus,
} from "@sershi/contracts";
import { useEffect, useState } from "react";

import { useI18n } from "../../i18n";
import { applicationName } from "../../i18n/domain";
import { desktopRuntime, sershi } from "../../ipc";
import { useConfirmation } from "../../state/confirmation";
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
          <Row
            label={t("settings.windows.shortcut")}
            detail={
              integration.shortcut.status === "unavailable"
                ? t("settings.windows.shortcutUnavailable")
                : t("settings.windows.shortcutDetail")
            }
            value={
              <span className={styles.pills}>
                <span className={styles.keys}>
                  {integration.shortcut.accelerator.split("+").map((key) => (
                    <kbd key={key}>{key}</kbd>
                  ))}
                </span>
                <Pill tone={TONE[integration.shortcut.status]}>
                  {t(`settings.featureStatus.${integration.shortcut.status}`)}
                </Pill>
              </span>
            }
          />
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

/** A sample used only by the Developer Mode preview; it can never be decided. */
function previewRequest(): ConfirmationRequest {
  return {
    id: "0".repeat(32),
    toolId: "system.close_application",
    action: "closeApplication",
    subject: {
      kind: "application",
      application: { id: "windows.notepad", displayName: "Notepad", source: "builtIn" },
    },
    risk: "sensitive",
    reason: "permissionUndecided",
    canRemember: false,
    expiresAtMs: Date.now() + 90_000,
  };
}

/** Developer Mode: what discovery found (names and sources, never paths). */
export function CatalogInspector() {
  const { t } = useI18n();
  const { catalog } = useWindowsIntegration();
  const showPreview = useConfirmation((s) => s.showPreview);
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
      <div className={styles.actions}>
        <button
          type="button"
          className={styles.secondary}
          onClick={() => {
            showPreview(previewRequest());
          }}
        >
          {t("settings.developer.previewConfirmation")}
        </button>
      </div>
    </Section>
  );
}
