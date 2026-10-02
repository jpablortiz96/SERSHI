//! What Settings shows about natural command understanding.

use serde::{Deserialize, Serialize};

use super::models::SemanticModel;
use crate::voice::ModelState;

/// The local semantic model, as Settings describes it (no paths).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SemanticModelInfo {
    pub id: String,
    pub name: String,
    pub quantization: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size_bytes: u64,
    pub sha256: String,
    pub license: String,
    /// Memory while loaded (measured), MB.
    pub ram_mb: u32,
    pub vram_mb: u32,
    pub state: ModelState,
}

impl SemanticModelInfo {
    pub fn of(model: &SemanticModel, state: ModelState, gpu: bool) -> Self {
        Self {
            id: model.id.to_owned(),
            name: model.name.to_owned(),
            quantization: model.quantization.to_owned(),
            size_bytes: model.size_bytes,
            sha256: model.sha256.to_owned(),
            license: model.license.to_owned(),
            ram_mb: if gpu {
                model.ram_mb_gpu
            } else {
                model.ram_mb_cpu
            },
            vram_mb: if gpu { model.vram_mb } else { 0 },
            state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SemanticStatus {
    pub model: SemanticModelInfo,
    /// The user's choice (on by default once installed).
    pub enabled: bool,
    /// Whether this computer can run it (platform and runtime present).
    pub available: bool,
    /// The model is loaded right now.
    pub running: bool,
    /// "vulkan" or "cpu" once loaded.
    pub backend: Option<String>,
    pub device: Option<String>,
    pub load_ms: Option<u32>,
    pub last_ms: Option<u32>,
    /// The model's volume has room for it plus a safety margin (a download
    /// is refused otherwise).
    pub free_space_ok: bool,
    /// Not loaded because another local model covers its role (the
    /// semantic router while the Agent Brain is active).
    pub standby: bool,
}

/// The only setting a surface can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticSettings {
    pub enabled: bool,
}
