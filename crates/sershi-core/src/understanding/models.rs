//! The local semantic model SERSHI can install (Gate 3C).
//!
//! Like speech models, it is **data**: downloaded only when the user asks,
//! from one pinned revision over HTTPS, checked for its exact size and
//! SHA-256, and moved into place atomically. It is never executed; the
//! `sershi-semantic` runtime reads it.
//!
//! Chosen by benchmark (docs/SEMANTIC.md): Qwen3 1.7B at Q4_K_M met every
//! quality requirement with half the memory and latency of Phi-4-mini
//! (3.8B), which scored higher on its own but no better through SERSHI's
//! policy; Qwen3 0.6B was too weak on natural paraphrases.

use super::semantic::ChatTemplate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticModel {
    /// Stable SERSHI id.
    pub id: &'static str,
    /// Human name (not translated: a product name).
    pub name: &'static str,
    /// Hugging Face repository and the commit the URL pins.
    pub repository: &'static str,
    pub revision: &'static str,
    /// File name in the model directory (fixed, never user-supplied).
    pub file_name: &'static str,
    pub quantization: &'static str,
    /// Exact download size in bytes.
    pub size_bytes: u64,
    /// Lower-case hex SHA-256 of the file.
    pub sha256: &'static str,
    /// SPDX license of the weights.
    pub license: &'static str,
    pub template: ChatTemplate,
    /// Measured on the reference machine: RAM of the engine process with
    /// the model on the GPU / on the CPU, and video memory on the GPU (MB).
    pub ram_mb_gpu: u32,
    pub ram_mb_cpu: u32,
    pub vram_mb: u32,
}

pub const SEMANTIC_MODELS: [SemanticModel; 1] = [SemanticModel {
    id: "qwen3-1.7b-q4km",
    name: "Qwen3 1.7B",
    repository: "unsloth/Qwen3-1.7B-GGUF",
    revision: "d7f544eead698dbd1f15126ef60b45a1e1933222",
    file_name: "Qwen3-1.7B-Q4_K_M.gguf",
    quantization: "Q4_K_M",
    size_bytes: 1_107_409_472,
    sha256: "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897",
    license: "Apache-2.0",
    template: ChatTemplate::ChatMl,
    ram_mb_gpu: 1_195,
    ram_mb_cpu: 1_444,
    vram_mb: 1_580,
}];

pub const DEFAULT_SEMANTIC_MODEL: &SemanticModel = &SEMANTIC_MODELS[0];

impl SemanticModel {
    pub fn find(id: &str) -> Option<&'static SemanticModel> {
        SEMANTIC_MODELS.iter().find(|m| m.id == id)
    }

    /// The fixed HTTPS download URL (pinned revision).
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repository, self.revision, self.file_name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_pinned_and_verifiable() {
        for m in SEMANTIC_MODELS {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(
                m.sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
            assert_eq!(m.revision.len(), 40, "a full commit, never a branch");
            assert!(m.revision.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(
                m.file_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            );
            assert!(m.file_name.ends_with(".gguf") && !m.file_name.contains(".."));
            assert!(m.file_name.contains(m.quantization));
            assert!(m.url().starts_with("https://huggingface.co/"));
            assert!(m.url().contains(m.revision));
            assert!(m.size_bytes > 100_000_000);
            assert!(!m.license.is_empty());
        }
        assert!(SemanticModel::find("../../evil").is_none());
        assert_eq!(
            SemanticModel::find("qwen3-1.7b-q4km").map(|m| m.id),
            Some(DEFAULT_SEMANTIC_MODEL.id)
        );
    }
}
