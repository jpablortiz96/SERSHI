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
import { Core } from "../../components/core/Core";
import { StateGlyph } from "../../components/core/StateGlyph";
import {
  COMPANION_SIZES,
  MOTION_PREFERENCES,
  THEME_PREFERENCES,
  useAppearance,
  type CompanionSize,
  type MotionPreference,
  type ThemePreference,
} from "../../visual/appearance";
import { companionRenderer } from "../../visual/companions";
import { playCue } from "../../audio/interfaceAudio";
import styles from "./Page.module.css";
import { SecuritySettings } from "./SecuritySettings";
import { Choices, Pill, Row, Section } from "./SettingsParts";
import { VoiceDiagnostics, VoiceSettings } from "./VoiceSettings";
import { UnderstandingDiagnostics, UnderstandingSettings } from "./UnderstandingSettings";
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
      </Section>

      <VoiceSettings />

      <UnderstandingSettings />

      <SecuritySettings />

      <Appearance />

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
          <VoiceDiagnostics />
          <UnderstandingDiagnostics />
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
            <Core state={s} variant="preview" size={28} decorative />
            <span className={styles.stateName}>
              <StateGlyph state={s} />
              {stateLabel(t, s)}
            </span>
          </button>
        ))}
      </div>
    </Section>
  );
}

/**
 * Appearance: only options that really work. Presentation only — stored with
 * the other interface preferences and applied in every SERSHI window.
 */
function Appearance() {
  const { t } = useI18n();
  const theme = useAppearance((s) => s.theme);
  const systemDark = useAppearance((s) => s.systemDark);
  const motion = useAppearance((s) => s.motion);
  const systemReduced = useAppearance((s) => s.systemReduced);
  const companionAppearance = useAppearance((s) => s.companionAppearance);
  const companionSize = useAppearance((s) => s.companionSize);
  const interfaceSounds = useAppearance((s) => s.interfaceSounds);
  const soundVolume = useAppearance((s) => s.soundVolume);
  const setTheme = useAppearance((s) => s.setTheme);
  const setMotion = useAppearance((s) => s.setMotion);
  const setCompanionSize = useAppearance((s) => s.setCompanionSize);
  const setInterfaceSounds = useAppearance((s) => s.setInterfaceSounds);
  const setSoundVolume = useAppearance((s) => s.setSoundVolume);

  return (
    <Section title={t("settings.sections.appearance")}>
      <Choices<ThemePreference>
        name="theme"
        label={t("settings.appearance.theme")}
        detail={t("settings.appearance.themeDetail")}
        options={THEME_PREFERENCES}
        value={theme}
        onChange={setTheme}
        optionLabel={(option) => t(`settings.appearance.themeOptions.${option}`)}
        optionDetail={(option) =>
          option === "system"
            ? t(
                systemDark
                  ? "settings.appearance.themeSystemDark"
                  : "settings.appearance.themeSystemLight",
              )
            : undefined
        }
      />
      <Choices<MotionPreference>
        name="motion"
        label={t("settings.appearance.motion")}
        detail={t("settings.appearance.motionDetail")}
        options={MOTION_PREFERENCES}
        value={motion}
        onChange={setMotion}
        optionLabel={(m) => t(`settings.appearance.motionOptions.${m}`)}
        optionDetail={(m) =>
          m === "system"
            ? t(
                systemReduced
                  ? "settings.appearance.motionSystemReduced"
                  : "settings.appearance.motionSystemFull",
              )
            : undefined
        }
      />
      {/* Only real renderers are listed; Orbital is the only one today. */}
      <Row
        label={t("settings.appearance.companion")}
        detail={t("settings.appearance.companionDetail")}
        value={t(companionRenderer(companionAppearance).label)}
      />
      <Choices<CompanionSize>
        name="companion-size"
        label={t("settings.appearance.companionSize")}
        detail={t("settings.appearance.companionSizeDetail")}
        options={COMPANION_SIZES}
        value={companionSize}
        onChange={setCompanionSize}
        optionLabel={(size) => t(`settings.appearance.sizes.${size}`)}
      />
      <Choices<"on" | "off">
        name="interface-sounds"
        label={t("settings.appearance.sounds")}
        detail={t("settings.appearance.soundsDetail")}
        options={["on", "off"]}
        value={interfaceSounds ? "on" : "off"}
        onChange={(value) => {
          setInterfaceSounds(value === "on");
        }}
        optionLabel={(value) => t(`settings.appearance.soundOptions.${value}`)}
      />
      <div className={styles.rowStacked}>
        <div>
          <label className={styles.rowLabel} htmlFor="sound-volume">
            {t("settings.appearance.volume")}
          </label>
          <p className={styles.rowDetail}>{t("settings.appearance.volumeDetail")}</p>
        </div>
        <div className={styles.volume}>
          <input
            id="sound-volume"
            type="range"
            min={0}
            max={100}
            step={5}
            value={soundVolume}
            disabled={!interfaceSounds}
            aria-valuetext={t("settings.appearance.volumeValue", { value: soundVolume })}
            onChange={(e) => {
              setSoundVolume(Number(e.target.value));
            }}
          />
          <output htmlFor="sound-volume" className="t-mono">
            {t("settings.appearance.volumeValue", { value: soundVolume })}
          </output>
          <button
            type="button"
            className={styles.secondary}
            disabled={!interfaceSounds || soundVolume === 0}
            onClick={() => {
              playCue("summon");
            }}
          >
            {t("settings.appearance.soundSample")}
          </button>
        </div>
      </div>
    </Section>
  );
}
