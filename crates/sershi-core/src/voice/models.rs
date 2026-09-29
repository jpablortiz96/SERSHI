//! The speech-recognition models SERSHI knows how to install.
//!
//! Models are **data**: SERSHI downloads them only when the user asks, from
//! a pinned revision of an approved source over HTTPS, verifies the exact
//! size and SHA-256, and only then moves them into place. They are never
//! executed. See docs/VOICE.md for sizes, memory use and the measured
//! trade-offs on the reference Windows machine.
//!
//! Source: the whisper.cpp project's GGML conversions of OpenAI Whisper
//! (MIT-licensed weights), `huggingface.co/ggerganov/whisper.cpp`, pinned to
//! one commit so a URL can never start serving different bytes unnoticed
//! (the SHA-256 check would reject them anyway).

use serde::Serialize;

/// The commit of `ggerganov/whisper.cpp` on Hugging Face the URLs pin.
pub const MODEL_REVISION: &str = "5359861c739e955e79d9a303bcbc70fb988958b1";

/// A downloadable speech-recognition model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SttModel {
    /// Stable SERSHI id, persisted in settings.
    pub id: &'static str,
    /// File name in the model directory (fixed, never user-supplied).
    pub file_name: &'static str,
    /// Exact download size in bytes.
    pub size_bytes: u64,
    /// Lower-case hex SHA-256 of the file.
    pub sha256: &'static str,
    /// Resident memory while loaded, in MB (measured on the reference
    /// machine: docs/VOICE.md#models).
    pub memory_mb: u32,
    pub tier: ModelTier,
}

/// How a model trades accuracy for speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ModelTier {
    /// Smallest and fastest; noticeably weaker outside English.
    Fast,
    /// SERSHI's default: good Spanish, English and Portuguese at usable speed.
    Balanced,
    /// Most accurate; slow on CPU-only machines.
    Accurate,
}

pub const DEFAULT_STT_MODEL: &str = "whisper-small-q8";

pub const STT_MODELS: [SttModel; 3] = [
    SttModel {
        id: "whisper-base-q8",
        file_name: "ggml-base-q8_0.bin",
        size_bytes: 81_768_585,
        sha256: "c577b9a86e7e048a0b7eada054f4dd79a56bbfa911fbdacf900ac5b567cbb7d9",
        memory_mb: 100,
        tier: ModelTier::Fast,
    },
    SttModel {
        id: DEFAULT_STT_MODEL,
        file_name: "ggml-small-q8_0.bin",
        size_bytes: 264_464_607,
        sha256: "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f",
        memory_mb: 300,
        tier: ModelTier::Balanced,
    },
    SttModel {
        id: "whisper-large-v3-turbo-q5",
        file_name: "ggml-large-v3-turbo-q5_0.bin",
        size_bytes: 574_041_195,
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        memory_mb: 600,
        tier: ModelTier::Accurate,
    },
];

impl SttModel {
    pub fn find(id: &str) -> Option<&'static SttModel> {
        STT_MODELS.iter().find(|m| m.id == id)
    }

    pub fn default_model() -> &'static SttModel {
        // The catalog always contains the default (tested below).
        Self::find(DEFAULT_STT_MODEL).unwrap_or(&STT_MODELS[1])
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
        assert!(SttModel::find(DEFAULT_STT_MODEL).is_some());
        assert_eq!(SttModel::default_model().tier, ModelTier::Balanced);
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
            assert!(
                m.url()
                    .starts_with("https://huggingface.co/ggerganov/whisper.cpp/resolve/")
            );
            assert!(m.url().contains(MODEL_REVISION));
            assert!(m.size_bytes > 10_000_000);
        }
    }

    #[test]
    fn unknown_models_are_not_found() {
        assert!(SttModel::find("../../evil").is_none());
    }
}
