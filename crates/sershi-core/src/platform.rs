//! Platform identity and capability reporting.
//!
//! Capabilities let every surface be honest about what works *here*: the UI
//! renders `requiresWindowsValidation` or `planned` instead of pretending a
//! feature exists.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
    Other,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum CapabilityStatus {
    /// Implemented and validated on this platform.
    Available,
    /// Implemented, but not yet validated on a physical Windows machine.
    RequiresWindowsValidation,
    /// On the roadmap; not implemented.
    Planned,
    /// Not supported on this platform.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapability {
    pub id: String,
    pub label: String,
    pub status: CapabilityStatus,
    /// Roadmap milestone for planned capabilities, e.g. `v0.3`.
    pub milestone: Option<String>,
}
