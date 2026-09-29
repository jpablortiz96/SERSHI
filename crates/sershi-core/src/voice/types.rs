//! Voice payloads that cross the UI ↔ core boundary.
//!
//! What the UI can *send* is small and validated: preferences (device,
//! language, voice, model ids) and start/stop requests. What it *receives*
//! is state, a transcript to display, a bounded 0–1 level for visuals, and
//! model-download progress. Raw audio never crosses IPC.

use serde::{Deserialize, Serialize};

use crate::service::CommandOutcome;

use super::language::LanguageTag;
use super::models::{ModelTier, STT_MODELS, SttModel};
use super::ports::{InputDevice, SynthesisVoice};

/// Longest accepted device or voice identifier.
pub const MAX_VOICE_ID: usize = 512;

/// The user's voice preferences, supplied by the Command Center (stored with
/// the other interface preferences). Invocation and presentation only:
/// nothing here is read by policy or permissions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceSettings {
    /// Microphone endpoint id; `None` is the system default.
    pub microphone: Option<String>,
    /// Conversation language; `None` is automatic detection.
    pub language: Option<LanguageTag>,
    /// Speech voice id; `None` picks a voice for the reply's language.
    pub voice: Option<String>,
    /// Speech model id; `None` is SERSHI's default.
    pub model: Option<String>,
}

impl VoiceSettings {
    /// Identifiers must be short printable text and the model must be one
    /// SERSHI knows. Anything else is refused, not repaired.
    pub fn is_valid(&self) -> bool {
        let id_ok = |id: &Option<String>| {
            id.as_ref().is_none_or(|v| {
                !v.is_empty() && v.len() <= MAX_VOICE_ID && !v.chars().any(char::is_control)
            })
        };
        id_ok(&self.microphone)
            && id_ok(&self.voice)
            && self
                .model
                .as_deref()
                .is_none_or(|m| SttModel::find(m).is_some())
    }

    pub fn model(&self) -> &'static SttModel {
        self.model
            .as_deref()
            .and_then(SttModel::find)
            .unwrap_or_else(SttModel::default_model)
    }
}

/// Whether Windows lets SERSHI use the microphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum MicrophoneAccess {
    Allowed,
    /// Microphone access is off for this user or for desktop apps.
    Denied,
    /// Could not be determined (the first capture will tell).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneStatus {
    pub access: MicrophoneAccess,
    pub devices: Vec<InputDevice>,
    /// The chosen device is not connected; the system default is used.
    pub fallback: bool,
}

/// Installation state of a speech model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ModelState {
    NotInstalled,
    Downloading {
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        received_bytes: u64,
    },
    Installed,
    /// The file on disk does not match; it is not loaded.
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub tier: ModelTier,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size_bytes: u64,
    pub memory_mb: u32,
    pub state: ModelState,
}

impl ModelInfo {
    pub fn catalog(state_of: impl Fn(&SttModel) -> ModelState) -> Vec<ModelInfo> {
        STT_MODELS
            .iter()
            .map(|m| ModelInfo {
                id: m.id.to_owned(),
                tier: m.tier,
                size_bytes: m.size_bytes,
                memory_mb: m.memory_mb,
                state: state_of(m),
            })
            .collect()
    }
}

/// Everything Settings and the command bar need to present voice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    /// Voice is implemented on this platform.
    pub supported: bool,
    pub microphone: MicrophoneStatus,
    pub models: Vec<ModelInfo>,
    /// The model voice input uses (installed or not).
    pub model: String,
    /// Installed system voices (empty if speech output is unavailable).
    pub voices: Vec<SynthesisVoice>,
    /// The microphone is capturing right now.
    pub capturing: bool,
    /// A spoken reply is playing right now.
    pub speaking: bool,
}

/// Why a voice interaction could not continue. Generic codes only: the UI
/// phrases them; internal details are never sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum VoiceFailure {
    Unsupported,
    NoMicrophone,
    PermissionDenied,
    /// The stream delivered only digital silence (typically privacy settings).
    MicrophoneSilent,
    DeviceLost,
    ModelMissing,
    ModelCorrupt,
    RecognitionFailed,
    SpeechUnavailable,
    OutputUnavailable,
    /// SERSHI is busy (working, or waiting for an approval it won't abandon).
    Busy,
}

/// The answer to a push-to-talk request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum CaptureStart {
    /// The microphone is open and SERSHI is listening.
    Started,
    /// The microphone was not opened.
    Refused { reason: VoiceFailure },
}

/// Voice events for the Command Center (`sershi://voice`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum VoiceUpdate {
    /// What SERSHI understood, shown before the command runs.
    Heard {
        text: String,
        language: Option<LanguageTag>,
    },
    /// The outcome of the heard command (same pipeline as typed commands).
    Answered {
        outcome: CommandOutcome,
    },
    NoSpeech,
    Unclear,
    Cancelled,
    Failed {
        reason: VoiceFailure,
    },
    /// The chosen microphone is missing; the system default was used.
    DeviceFallback,
}

/// Which way sound is flowing, for visuals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum LevelSource {
    Input,
    Output,
}

/// A bounded audio level for visuals only (`sershi://voice-level`),
/// throttled to at most ~25 per second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct VoiceLevel {
    /// 0–1.
    pub level: f32,
    pub source: LevelSource,
}

/// Model download progress (`sershi://voice-model`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ModelProgress {
    pub model: String,
    pub state: ModelState,
    /// Set when a download ended without installing the model.
    pub error: Option<ModelError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ModelError {
    Network,
    /// The downloaded file did not match the expected size or SHA-256.
    Integrity,
    Storage,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_reject_unknown_fields_and_bad_values() {
        assert!(
            serde_json::from_str::<VoiceSettings>(r#"{"microphone":null,"approve":true}"#).is_err()
        );
        assert!(serde_json::from_str::<VoiceSettings>(r#"{"language":"; rm"}"#).is_err());
        let ok: VoiceSettings =
            serde_json::from_str(r#"{"microphone":"{0.0.1}.{abc}","language":"es"}"#).unwrap();
        assert!(ok.is_valid());
        for bad in [
            VoiceSettings {
                model: Some("../../model".into()),
                ..VoiceSettings::default()
            },
            VoiceSettings {
                microphone: Some("x".repeat(MAX_VOICE_ID + 1)),
                ..VoiceSettings::default()
            },
            VoiceSettings {
                voice: Some("evil\u{202e}\n".into()),
                ..VoiceSettings::default()
            },
        ] {
            assert!(!bad.is_valid(), "{bad:?}");
        }
    }

    #[test]
    fn the_default_model_is_used_unless_another_is_chosen() {
        assert_eq!(
            VoiceSettings::default().model().id,
            super::super::DEFAULT_STT_MODEL
        );
    }

    #[test]
    fn updates_never_carry_audio_or_confirmation_ids() {
        let heard = serde_json::to_value(VoiceUpdate::Heard {
            text: "Abre Spotify".into(),
            language: LanguageTag::parse("es").ok(),
        })
        .unwrap();
        assert_eq!(heard["kind"], "heard");
        let keys: Vec<_> = heard.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys.len(), 3, "{keys:?}");
        let level = serde_json::to_value(VoiceLevel {
            level: 0.5,
            source: LevelSource::Input,
        })
        .unwrap();
        assert_eq!(level.as_object().unwrap().len(), 2);
    }
}
