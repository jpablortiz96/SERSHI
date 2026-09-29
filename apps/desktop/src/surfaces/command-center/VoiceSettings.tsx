import type { ModelInfo } from "@sershi/contracts";
import { useEffect } from "react";

import { downloadPercent } from "../../components/command/VoicePanel";
import { MESSAGES, useI18n } from "../../i18n";
import { modelName } from "../../i18n/domain";
import { CONVERSATION_LANGUAGES, type ConversationLanguage, type Locale } from "../../i18n/types";
import { desktopRuntime } from "../../ipc";
import { useVoice } from "../../state/voice";
import styles from "./Page.module.css";
import { Choices, Pill, Row, Section } from "./SettingsParts";

/** Endonyms come from each locale's own messages. */
const LANGUAGE_LOCALE: Record<Exclude<ConversationLanguage, "automatic">, Locale> = {
  en: "en-US",
  es: "es-419",
  pt: "pt-BR",
};

/**
 * Voice: only what actually exists. Push-to-talk, conversation language,
 * spoken replies, the Windows voice and the local speech model. Every value
 * is a preference; none can approve anything or grant a permission.
 */
export function VoiceSettings() {
  const { t } = useI18n();
  const status = useVoice((s) => s.status);
  const prefs = useVoice((s) => s.prefs);
  const setPrefs = useVoice((s) => s.setPrefs);
  const refresh = useVoice((s) => s.refresh);

  // Opening Settings re-reads microphones and voices (they change at runtime).
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const supported = desktopRuntime && status?.supported === true;

  return (
    <Section title={t("settings.sections.voice")}>
      <Choices<ConversationLanguage>
        name="conversation-language"
        label={t("settings.voice.language")}
        detail={t("settings.voice.languageDetail")}
        options={CONVERSATION_LANGUAGES}
        value={prefs.conversationLanguage}
        onChange={(conversationLanguage) => {
          setPrefs({ conversationLanguage });
        }}
        optionLabel={(option) =>
          option === "automatic"
            ? t("settings.voice.languageAutomatic")
            : MESSAGES[LANGUAGE_LOCALE[option]].meta.languageName
        }
      />
      {supported ? (
        <>
          <Microphone />
          <Choices<"on" | "off">
            name="voice-responses"
            label={t("settings.voice.responses")}
            detail={t("settings.voice.responsesDetail")}
            options={["on", "off"]}
            value={prefs.voiceResponses ? "on" : "off"}
            onChange={(value) => {
              setPrefs({ voiceResponses: value === "on" });
            }}
            optionLabel={(value) => t(`settings.voice.options.${value}`)}
          />
          <Choices<"on" | "off">
            name="speak-typed"
            label={t("settings.voice.speakTyped")}
            detail={t("settings.voice.speakTypedDetail")}
            options={["on", "off"]}
            value={prefs.speakTypedResponses ? "on" : "off"}
            onChange={(value) => {
              setPrefs({ speakTypedResponses: value === "on" });
            }}
            optionLabel={(value) => t(`settings.voice.options.${value}`)}
          />
          <SpeechVoice />
          <Models />
        </>
      ) : (
        <p className={styles.empty}>{t("settings.voice.unsupported")}</p>
      )}
      <Row label={t("settings.voice.wakeWord")} value={t("settings.voice.wakeWordValue")} />
      <p className={styles.footnote}>{t("settings.voice.privacy")}</p>
    </Section>
  );
}

function Microphone() {
  const { t } = useI18n();
  const status = useVoice((s) => s.status);
  const microphone = useVoice((s) => s.prefs.microphone);
  const setPrefs = useVoice((s) => s.setPrefs);
  if (!status) return null;
  const { devices, access, fallback } = status.microphone;
  return (
    <div className={styles.rowStacked}>
      <div>
        <label className={styles.rowLabel} htmlFor="voice-microphone">
          {t("settings.voice.microphone")}
        </label>
        <p className={styles.rowDetail}>{t("settings.voice.microphoneDetail")}</p>
        {access === "denied" && (
          <p className={styles.rowDetail} role="alert">
            <Pill tone="warning">{t("settings.voice.accessDenied")}</Pill>
          </p>
        )}
        {fallback && (
          <p className={styles.rowDetail} role="status">
            <Pill tone="warning">{t("settings.voice.fallback")}</Pill>
          </p>
        )}
        {devices.length === 0 && (
          <p className={styles.rowDetail}>{t("settings.voice.noDevices")}</p>
        )}
      </div>
      <select
        id="voice-microphone"
        className={styles.select}
        value={microphone ?? ""}
        onChange={(e) => {
          setPrefs({ microphone: e.target.value || null });
        }}
      >
        <option value="">{t("settings.voice.systemDefault")}</option>
        {devices.map((d) => (
          <option key={d.id} value={d.id}>
            {d.name}
          </option>
        ))}
        {/* A remembered device that is unplugged stays selectable. */}
        {microphone && !devices.some((d) => d.id === microphone) && (
          <option value={microphone}>{t("settings.voice.fallback")}</option>
        )}
      </select>
    </div>
  );
}

function SpeechVoice() {
  const { t } = useI18n();
  const voices = useVoice((s) => s.status?.voices ?? []);
  const speechVoice = useVoice((s) => s.prefs.speechVoice);
  const setPrefs = useVoice((s) => s.setPrefs);
  return (
    <div className={styles.rowStacked}>
      <div>
        <label className={styles.rowLabel} htmlFor="voice-voice">
          {t("settings.voice.voice")}
        </label>
        <p className={styles.rowDetail}>
          {voices.length ? t("settings.voice.voiceDetail") : t("settings.voice.noVoices")}
        </p>
      </div>
      <select
        id="voice-voice"
        className={styles.select}
        value={voices.some((v) => v.id === speechVoice) ? (speechVoice ?? "") : ""}
        disabled={voices.length === 0}
        onChange={(e) => {
          setPrefs({ speechVoice: e.target.value || null });
        }}
      >
        <option value="">{t("settings.voice.systemDefault")}</option>
        {voices.map((v) => (
          <option key={v.id} value={v.id}>
            {`${v.name} · ${v.language}`}
          </option>
        ))}
      </select>
    </div>
  );
}

function Models() {
  const { t } = useI18n();
  const status = useVoice((s) => s.status);
  if (!status) return null;
  const downloading = status.models.some((m) => m.state.kind === "downloading");
  return (
    <div className={styles.rowStacked}>
      <div>
        <p className={styles.rowLabel}>{t("settings.voice.model")}</p>
        <p className={styles.rowDetail}>{t("settings.voice.modelDetail")}</p>
      </div>
      <ul className={styles.models}>
        {status.models.map((m) => (
          <ModelRow key={m.id} model={m} active={m.id === status.model} busy={downloading} />
        ))}
      </ul>
    </div>
  );
}

function ModelRow({ model, active, busy }: { model: ModelInfo; active: boolean; busy: boolean }) {
  const { t, format } = useI18n();
  const setPrefs = useVoice((s) => s.setPrefs);
  const download = useVoice((s) => s.download);
  const cancelDownload = useVoice((s) => s.cancelDownload);
  const percent = downloadPercent(model);
  const installed = model.state.kind === "installed";

  let action;
  if (percent !== null) {
    action = (
      <button type="button" className={styles.secondary} onClick={cancelDownload}>
        {t("voice.model.cancel")}
      </button>
    );
  } else if (installed) {
    action = (
      <button
        type="button"
        className={styles.secondary}
        aria-pressed={active}
        disabled={active}
        onClick={() => {
          setPrefs({ speechModel: model.id });
        }}
      >
        {active ? t("settings.voice.inUse") : t("settings.voice.use")}
      </button>
    );
  } else {
    action = (
      <button
        type="button"
        className={styles.secondary}
        disabled={busy}
        onClick={() => {
          setPrefs({ speechModel: model.id });
          download(model.id);
        }}
      >
        {t("voice.model.action")}
      </button>
    );
  }

  return (
    <li className={styles.model} data-active={active || undefined}>
      <div>
        <p className={styles.rowLabel}>{modelName(t, model.id)}</p>
        <p className={styles.rowDetail}>
          {t(`settings.voice.tiers.${model.tier}`)} ·{" "}
          {t("settings.voice.modelMeta", {
            size: format.megabytes(model.sizeBytes),
            memory: format.megabytes(model.memoryMb * 1_000_000),
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
      </div>
      <div className={styles.modelActions}>
        {installed && <Pill tone="success">{t("settings.voice.installed")}</Pill>}
        {model.state.kind === "corrupt" && (
          <Pill tone="warning">{t("settings.voice.corrupt")}</Pill>
        )}
        {action}
      </div>
    </li>
  );
}
