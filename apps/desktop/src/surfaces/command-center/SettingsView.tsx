import { ASSISTANT_STATES, type AssistantState } from "@sershi/contracts";
import {
  MESSAGES,
  SUPPORTED_LOCALES,
  useI18n,
  useLocaleStore,
  type LocalePreference,
} from "../../i18n";
import {
  capabilityLabelFor,
  capabilityStatus,
  platformName,
  riskLabel,
  stateLabel,
  toolName,
} from "../../i18n/domain";
import { desktopRuntime, sershi } from "../../ipc";
import { useAssistantStore } from "../../state/assistant";
import { useRuntimeInfo } from "../../state/runtime";
import styles from "./Page.module.css";
import { Pill, Row, Section } from "./SettingsParts";
import { CatalogInspector, WindowsIntegration } from "./WindowsIntegration";

export function SettingsView() {
  const info = useRuntimeInfo();
  const { t } = useI18n();
  const developerMode = info?.developerBuild ?? !desktopRuntime;
  const desktopOnly = <p className={styles.empty}>{t("settings.desktopOnly")}</p>;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>{t("settings.title")}</h1>
        <p className={styles.lede}>{t("settings.lede")}</p>
      </header>

      <Section title={t("settings.sections.general")}>
        <LanguagePicker />
        <Row
          label={t("settings.conversation.label")}
          detail={t("settings.conversation.detail")}
          value={t("settings.conversation.value")}
        />
      </Section>

      <WindowsIntegration />

      <Section title={t("settings.sections.privacy")}>
        <Row label={t("settings.privacy.typed")} value={t("settings.privacy.typedValue")} />
        <Row
          label={t("settings.privacy.conversation")}
          value={t("settings.privacy.conversationValue")}
        />
        <Row label={t("settings.privacy.devices")} value={t("settings.privacy.devicesValue")} />
        <Row label={t("settings.privacy.analytics")} value={t("settings.privacy.analyticsValue")} />
      </Section>

      <Section title={t("settings.sections.tools")}>
        {info
          ? info.tools.map((tool) => (
              <Row
                key={tool.id}
                label={toolName(t, tool.id, tool.name)}
                detail={<span className="t-mono">{tool.id}</span>}
                value={
                  <span className={styles.pills}>
                    <Pill tone={tool.risk === "safe" ? "success" : "warning"}>
                      {riskLabel(t, tool.risk)}
                    </Pill>
                    <span className="t-mono">{tool.permissions.join(", ")}</span>
                  </span>
                }
              />
            ))
          : desktopOnly}
        <p className={styles.footnote}>{t("settings.toolsFootnote")}</p>
      </Section>

      <Section title={t("settings.sections.platform")}>
        {info
          ? info.capabilities.map((c) => (
              <Row
                key={c.id}
                label={capabilityLabelFor(t, c)}
                value={
                  <Pill tone={c.status === "available" ? "success" : "neutral"}>
                    {capabilityStatus(t, c.status)}
                    {c.milestone ? ` · ${c.milestone}` : ""}
                  </Pill>
                }
              />
            ))
          : desktopOnly}
      </Section>

      {developerMode && (
        <>
          <StatePreview />
          <CatalogInspector />
        </>
      )}

      <Section title={t("settings.sections.about")}>
        <Row
          label={t("settings.about.version")}
          value={
            info
              ? t("settings.about.versionValue", { version: info.version })
              : t("settings.about.browserPreview")
          }
        />
        <Row
          label={t("settings.about.platform")}
          value={info ? platformName(t, info.platform) : "—"}
        />
        <Row label={t("settings.about.license")} value="Apache-2.0" />
        {desktopRuntime && (
          <div className={styles.actions}>
            <button
              type="button"
              className={styles.danger}
              onClick={() => {
                sershi.quit().catch(() => undefined);
              }}
            >
              {t("settings.about.quit")}
            </button>
          </div>
        )}
      </Section>
    </div>
  );
}

const LANGUAGE_OPTIONS: readonly LocalePreference[] = ["auto", ...SUPPORTED_LOCALES];

/**
 * Interface language. A manual choice overrides the system language and is
 * remembered; "Automatic" follows the operating system again.
 */
function LanguagePicker() {
  const { t } = useI18n();
  const preference = useLocaleStore((s) => s.preference);
  const systemLocale = useLocaleStore((s) => s.systemLocale);
  const setPreference = useLocaleStore((s) => s.setPreference);

  return (
    <div className={styles.rowStacked}>
      <div>
        <p className={styles.rowLabel} id="language-label">
          {t("settings.language.label")}
        </p>
        <p className={styles.rowDetail}>{t("settings.language.detail")}</p>
      </div>
      <div className={styles.choices} role="radiogroup" aria-labelledby="language-label">
        {LANGUAGE_OPTIONS.map((option) => {
          const auto = option === "auto";
          return (
            <label key={option} className={styles.choice}>
              <input
                type="radio"
                name="ui-locale"
                value={option}
                checked={preference === option}
                onChange={() => {
                  setPreference(option);
                }}
              />
              <span className={styles.choiceText}>
                <span lang={auto ? undefined : option}>
                  {auto ? t("settings.language.automatic") : MESSAGES[option].meta.languageName}
                </span>
                {auto && (
                  <span className={styles.choiceDetail}>
                    {t("settings.language.automaticDetail", {
                      language: MESSAGES[systemLocale].meta.languageName,
                    })}
                  </span>
                )}
              </span>
            </label>
          );
        })}
      </div>
    </div>
  );
}

function StatePreview() {
  const { t } = useI18n();
  const serverPreview = useAssistantStore((s) => s.snapshot.previewState);
  const localPreview = useAssistantStore((s) => s.localPreview);
  const setLocalPreview = useAssistantStore((s) => s.setLocalPreview);
  const active = localPreview ?? serverPreview;

  const preview = (state: AssistantState | null) => {
    if (desktopRuntime) sershi.previewState(state).catch(() => undefined);
    else setLocalPreview(state);
  };

  return (
    <Section title={t("settings.sections.developer")}>
      <p className={styles.footnote}>{t("settings.developer.footnote")}</p>
      <div
        className={styles.stateGrid}
        role="group"
        aria-label={t("settings.developer.groupLabel")}
      >
        <button
          type="button"
          aria-pressed={active === null}
          onClick={() => {
            preview(null);
          }}
        >
          {t("settings.developer.live")}
        </button>
        {ASSISTANT_STATES.map((s) => (
          <button
            key={s}
            type="button"
            data-state={s}
            aria-pressed={active === s}
            onClick={() => {
              preview(s);
            }}
          >
            <i aria-hidden="true" />
            {stateLabel(t, s)}
          </button>
        ))}
      </div>
    </Section>
  );
}
