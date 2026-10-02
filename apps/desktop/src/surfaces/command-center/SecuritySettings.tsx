import { useEffect } from "react";

import type { PermissionSetting } from "@sershi/contracts";

import { useI18n } from "../../i18n";
import { permissionLabel } from "../../i18n/domain";
import { desktopRuntime } from "../../ipc";
import { connectPermissions, usePermissions } from "../../state/permissions";
import styles from "./Page.module.css";
import { Choices, Pill, Section } from "./SettingsParts";

const SETTINGS = ["askEveryTime", "alwaysAllow"] as const satisfies readonly PermissionSetting[];

/**
 * Settings › Security & permissions (Gate 4.1). Hands-free comes from what
 * the user allows here, deliberately — never from a spoken "yes". Choosing
 * "Always allow" opens the trusted confirmation window; only approving it
 * there changes the setting. "Ask every time" applies at once.
 */
export function SecuritySettings() {
  const { t } = useI18n();
  const { settings, pending, notice, request, apps, pendingApp, requestApp } = usePermissions();

  useEffect(connectPermissions, []);

  return (
    <Section title={t("settings.sections.security")}>
      <p className={styles.lede}>{t("settings.security.lede")}</p>
      {!desktopRuntime || !settings ? (
        <p className={styles.empty}>{t("settings.desktopOnly")}</p>
      ) : (
        settings.map((status) => (
          <div key={status.permission} data-permission={status.permission}>
            <Choices<PermissionSetting>
              name={`permission-${status.permission}`}
              label={permissionLabel(t, status.permission)}
              detail={t(`settings.security.permissions.${status.permission}.detail`)}
              options={SETTINGS}
              value={status.setting}
              onChange={(setting) => {
                void request(status.permission, setting);
              }}
              optionLabel={(setting) =>
                setting === status.defaultSetting
                  ? `${t(`settings.security.settings.${setting}`)} · ${t("settings.security.recommended")}`
                  : t(`settings.security.settings.${setting}`)
              }
              optionDetail={(setting) =>
                setting === "alwaysAllow"
                  ? t("settings.security.alwaysAllowDetail")
                  : t("settings.security.askEveryTimeDetail")
              }
            />
            {pending === status.permission && notice === "pending" && (
              <p className={styles.rowDetail} role="status">
                <Pill tone="neutral">{t("settings.security.pending")}</Pill>
              </p>
            )}
          </div>
        ))
      )}
      {desktopRuntime && settings && (
        <div className={styles.rowStacked} data-section="application-permissions">
          <div>
            <p className={styles.rowLabel}>{t("settings.security.apps.title")}</p>
            <p className={styles.rowDetail}>{t("settings.security.apps.detail")}</p>
          </div>
        </div>
      )}
      {desktopRuntime && settings && apps.length === 0 && (
        <p className={styles.empty}>{t("settings.security.apps.empty")}</p>
      )}
      {desktopRuntime &&
        settings &&
        apps.map((app) => (
          <div key={app.appId} data-app={app.appId}>
            <Choices<PermissionSetting>
              name={`app-permission-${app.appId}`}
              label={app.displayName}
              detail={
                app.closeRisk === "safeToClose"
                  ? t("settings.security.apps.safe")
                  : app.setting === null
                    ? t("settings.security.apps.defaultDetail")
                    : t("settings.security.permissions.closeApplications.label")
              }
              options={SETTINGS}
              value={app.setting ?? "askEveryTime"}
              onChange={(setting) => {
                void requestApp(app.appId, setting);
              }}
              optionLabel={(setting) => t(`settings.security.settings.${setting}`)}
              optionDetail={(setting) =>
                setting === "alwaysAllow"
                  ? t("settings.security.apps.alwaysAllowDetail")
                  : t("settings.security.apps.askEveryTimeDetail")
              }
            />
            {pendingApp === app.appId && notice === "pending" && (
              <p className={styles.rowDetail} role="status">
                <Pill tone="neutral">{t("settings.security.pending")}</Pill>
              </p>
            )}
          </div>
        ))}
      {notice === "busy" && (
        <p className={styles.rowDetail} role="alert">
          <Pill tone="warning">{t("settings.security.busy")}</Pill>
        </p>
      )}
      <p className={styles.footnote}>{t("settings.security.footnote")}</p>
    </Section>
  );
}
