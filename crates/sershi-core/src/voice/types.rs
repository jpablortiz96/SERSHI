//! Voice payloads that cross the UI ↔ core boundary.
//!
//! What the UI can *send* is small and validated: preferences (device,
//! language, voice, speech profile) and start/stop requests. What it *receives*
//! is state, a transcript to display, a bounded 0–1 level for visuals, and
//! model-download progress. Raw audio never crosses IPC.

use serde::{Deserialize, Serialize};

use crate::service::CommandOutcome;

use super::language::LanguageTag;
use super::latency::{Acceleration, VoiceTimings};
use super::models::{DEFAULT_PROFILE, STT_MODELS, SpeechProfile, SttModel};
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
    /// Speech recognition profile; `None` is SERSHI's default (Fast).
    pub profile: Option<SpeechProfile>,
    /// Developer tuning: one fixed end-of-speech silence instead of the
    /// adaptive default. Accepted range [`ENDPOINT_RANGE_MS`].
    pub endpoint_ms: Option<u32>,
    /// The interface language (what SERSHI writes and speaks). Only a hint
    /// for choosing a second recognition pass's language in Automatic mode
    /// (Gate 3C.1); it never forces recognition.
    #[serde(default)]
    pub interface_language: Option<LanguageTag>,
}

/// Accepted developer endpoint override, in milliseconds.
pub const ENDPOINT_RANGE_MS: std::ops::RangeInclusive<u32> = 300..=1_500;

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
                .endpoint_ms
                .is_none_or(|ms| ENDPOINT_RANGE_MS.contains(&ms))
    }

    pub fn profile(&self) -> SpeechProfile {
        self.profile.unwrap_or(DEFAULT_PROFILE)
    }

    pub fn model(&self) -> &'static SttModel {
        SttModel::for_profile(self.profile())
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
    pub profile: SpeechProfile,
    pub file_name: String,
    pub quantization: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size_bytes: u64,
    pub sha256: String,
    pub memory_mb: u32,
    pub state: ModelState,
}

impl ModelInfo {
    pub fn catalog(state_of: impl Fn(&SttModel) -> ModelState) -> Vec<ModelInfo> {
        STT_MODELS
            .iter()
            .map(|m| ModelInfo {
                id: m.id.to_owned(),
                profile: m.profile,
                file_name: m.file_name.to_owned(),
                quantization: m.quantization.to_owned(),
                size_bytes: m.size_bytes,
                sha256: m.sha256.to_owned(),
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
    /// The chosen speech profile.
    pub profile: SpeechProfile,
    /// The model voice input uses (installed or not).
    pub model: String,
    /// Where recognition runs; the CPU is always the fallback.
    pub acceleration: Acceleration,
    /// The accelerator's name (e.g. the GPU), when one is used.
    pub accelerator: Option<String>,
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
        /// When a second pass replaced a suspected language misdetection:
        /// what the first pass literally heard, shown next to the accepted
        /// text (never hidden, never logged).
        #[serde(default)]
        first_heard: Option<String>,
    },
    /// The outcome of the heard command (same pipeline as typed commands).
    Answered {
        outcome: Box<CommandOutcome>,
        /// Speak the reply. False when the user already moved on (they
        /// interrupted, a newer turn began): it is shown, never spoken.
        speak: bool,
        /// In a voice session: follow the reply with "anything else?"
        /// (Gate 4.1; sparingly, never twice in a row).
        anything_else: bool,
    },
    /// In a voice session, "sí" to "anything else?": SERSHI says it is
    /// listening. Nothing was submitted.
    Prompt,
    NoSpeech,
    Unclear,
    Cancelled,
    Failed {
        reason: VoiceFailure,
    },
    /// The chosen microphone is missing; the system default was used.
    DeviceFallback,
    /// Where the time went for the last spoken command (diagnostics; never
    /// what was said).
    Timings {
        timings: VoiceTimings,
    },
    /// The spoken reply started this long after the outcome arrived.
    SpeechLatency {
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        ms: u32,
    },
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
    /// Not enough free disk space for the model and a safety margin; the
    /// download did not start.
    DiskFull,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_reject_unknown_fields_and_bad_values() {
        assert!(
            serde_json::from_str::<VoiceSettings>(r#"{"microphone":null,"approve":true}"#).is_err()
        );
        assert!(serde_json::from_str::<VoiceSettings>(r#"{"profile":"turbo"}"#).is_err());
        assert!(serde_json::from_str::<VoiceSettings>(r#"{"model":"../../model"}"#).is_err());
        let accurate: VoiceSettings = serde_json::from_str(r#"{"profile":"accurate"}"#).unwrap();
        assert_eq!(accurate.model().id, "whisper-large-v3-turbo-q8");
        assert!(serde_json::from_str::<VoiceSettings>(r#"{"language":"; rm"}"#).is_err());
        let ok: VoiceSettings =
            serde_json::from_str(r#"{"microphone":"{0.0.1}.{abc}","language":"es"}"#).unwrap();
        assert!(ok.is_valid());
        for bad in [
            VoiceSettings {
                endpoint_ms: Some(100),
                ..VoiceSettings::default()
            },
            VoiceSettings {
                endpoint_ms: Some(10_000),
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
        assert_eq!(VoiceSettings::default().model().id, "whisper-small-q8");
        assert_eq!(VoiceSettings::default().profile(), SpeechProfile::Fast);
    }

    #[test]
    fn updates_never_carry_audio_or_confirmation_ids() {
        let heard = serde_json::to_value(VoiceUpdate::Heard {
            text: "Ponme Chrome".into(),
            language: LanguageTag::parse("es").ok(),
            first_heard: Some("Пон Мекром".into()),
        })
        .unwrap();
        assert_eq!(heard["kind"], "heard");
        let keys: Vec<_> = heard.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys.len(), 4, "{keys:?}");
        // Words only as text the user sees; nothing else.
        assert_eq!(heard["firstHeard"], "Пон Мекром");
        let level = serde_json::to_value(VoiceLevel {
            level: 0.5,
            source: LevelSource::Input,
        })
        .unwrap();
        assert_eq!(level.as_object().unwrap().len(), 2);
    }
}
