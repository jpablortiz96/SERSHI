//! The speech-recognition models SERSHI knows how to install, and the two
//! profiles the user chooses between.
//!
//! Models are **data**: SERSHI downloads them only when the user asks, from
//! a pinned revision of an approved source over HTTPS, verifies the exact
//! size and SHA-256, and only then moves them into place. They are never
//! executed. See docs/VOICE.md#models for sizes, memory use and the measured
//! trade-offs on the reference Windows machine.
//!
//! Source: the whisper.cpp project's GGML conversions of OpenAI Whisper
//! (MIT-licensed weights), `huggingface.co/ggerganov/whisper.cpp`, pinned to
//! one commit so a URL can never start serving different bytes unnoticed
//! (the SHA-256 check would reject them anyway).
//!
//! Only profiles backed by benchmarks exist (Gate 3B): Whisper Base was
//! dropped because it mis-heard Spanish commands (even as Greek, with high
//! confidence), and the q5_0 Turbo quantization because q8_0 is faster on
//! the GPU at the same accuracy.

use serde::{Deserialize, Serialize};

/// The commit of `ggerganov/whisper.cpp` on Hugging Face the URLs pin.
pub const MODEL_REVISION: &str = "5359861c739e955e79d9a303bcbc70fb988958b1";

/// What the user chooses in Settings › Voice. Each profile maps to exactly
/// one model; the labels describe measured behaviour, not marketing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum SpeechProfile {
    /// Lowest latency with good command accuracy (the default).
    Fast,
    /// Best accuracy (dictation, long questions); needs GPU acceleration to
    /// be interactive.
    Accurate,
}

impl SpeechProfile {
    pub const ALL: [SpeechProfile; 2] = [Self::Fast, Self::Accurate];
}

/// A downloadable speech-recognition model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SttModel {
    /// Stable SERSHI id.
    pub id: &'static str,
    pub profile: SpeechProfile,
    /// File name in the model directory (fixed, never user-supplied).
    pub file_name: &'static str,
    /// Weight quantization, as named by ggml.
    pub quantization: &'static str,
    /// Exact download size in bytes.
    pub size_bytes: u64,
    /// Lower-case hex SHA-256 of the file.
    pub sha256: &'static str,
    /// Memory while loaded, in MB (measured; system RAM on CPU, mostly
    /// GPU memory with acceleration).
    pub memory_mb: u32,
}

pub const DEFAULT_PROFILE: SpeechProfile = SpeechProfile::Fast;

pub const STT_MODELS: [SttModel; 2] = [
    SttModel {
        id: "whisper-small-q8",
        profile: SpeechProfile::Fast,
        file_name: "ggml-small-q8_0.bin",
        quantization: "q8_0",
        size_bytes: 264_464_607,
        sha256: "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f",
        memory_mb: 300,
    },
    SttModel {
        id: "whisper-large-v3-turbo-q8",
        profile: SpeechProfile::Accurate,
        file_name: "ggml-large-v3-turbo-q8_0.bin",
        quantization: "q8_0",
        size_bytes: 874_188_075,
        sha256: "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
        memory_mb: 950,
    },
];

impl SttModel {
    pub fn find(id: &str) -> Option<&'static SttModel> {
        STT_MODELS.iter().find(|m| m.id == id)
    }

    /// The model behind a profile (every profile has exactly one).
    pub fn for_profile(profile: SpeechProfile) -> &'static SttModel {
        STT_MODELS
            .iter()
            .find(|m| m.profile == profile)
            .unwrap_or(&STT_MODELS[0])
    }

    /// The fixed HTTPS download URL (pinned revision).
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/{MODEL_REVISION}/{}",
            self.file_name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        for m in STT_MODELS {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(
                m.sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
            // File names are fixed, simple and cannot traverse directories.
            assert!(
                m.file_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            );
            assert!(m.file_name.ends_with(".bin") && !m.file_name.contains(".."));
            assert!(m.file_name.contains(m.quantization), "{}", m.id);
            assert!(
                m.url()
                    .starts_with("https://huggingface.co/ggerganov/whisper.cpp/resolve/")
            );
            assert!(m.url().contains(MODEL_REVISION));
            assert!(m.size_bytes > 10_000_000);
        }
    }

    #[test]
    fn every_profile_has_exactly_one_model() {
        for profile in SpeechProfile::ALL {
            let models: Vec<_> = STT_MODELS.iter().filter(|m| m.profile == profile).collect();
            assert_eq!(models.len(), 1, "{profile:?}");
            assert_eq!(SttModel::for_profile(profile).profile, profile);
        }
        assert_eq!(
            SttModel::for_profile(DEFAULT_PROFILE).id,
            "whisper-small-q8"
        );
    }

    #[test]
    fn ids_and_files_are_unique() {
        for (i, a) in STT_MODELS.iter().enumerate() {
            for b in &STT_MODELS[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.file_name, b.file_name);
                assert_ne!(a.sha256, b.sha256);
            }
        }
    }

    #[test]
    fn unknown_models_are_not_found() {
        assert!(SttModel::find("../../evil").is_none());
        assert!(
            SttModel::find("whisper-base-q8").is_none(),
            "dropped in Gate 3B"
        );
    }
}
