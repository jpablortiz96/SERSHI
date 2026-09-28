//! Payloads that exist only to cross the UI ↔ core boundary.
//!
//! Command and event *names* live in the desktop shell; the TypeScript side
//! mirrors them in `packages/contracts/src/commands.ts`.

use serde::Serialize;

use crate::platform::{Platform, PlatformCapability};
use crate::tool::ToolDefinition;

/// Error returned by any IPC command. `message` is safe to show to users;
/// it never contains paths, payloads or secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum IpcErrorCode {
    Unavailable,
    Internal,
    NotAllowed,
}

impl IpcError {
    pub fn new(code: IpcErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Static facts about this running instance.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub version: String,
    pub platform: Platform,
    /// True in debug builds; enables Developer Mode affordances.
    pub developer_build: bool,
    pub capabilities: Vec<PlatformCapability>,
    pub tools: Vec<ToolDefinition>,
}

/// Whether an OS integration is working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum FeatureStatus {
    Active,
    /// Implemented but could not be enabled (e.g. another app owns the
    /// shortcut, or the platform has no tray).
    Unavailable,
    Planned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    /// Accelerator in `Modifier+Key` form, e.g. `Ctrl+Alt+Space`.
    pub accelerator: String,
    pub status: FeatureStatus,
}

/// Windows integration state for Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub tray: FeatureStatus,
    pub shortcut: ShortcutStatus,
    pub single_instance: FeatureStatus,
    pub start_with_windows: FeatureStatus,
}

/// Localized tray menu labels supplied by the UI (presentation only: the
/// actions behind each item are fixed in Rust).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrayLabels {
    pub open: String,
    pub hide: String,
    pub quit: String,
}

/// Longest accepted tray label.
pub const MAX_TRAY_LABEL: usize = 48;

impl TrayLabels {
    /// Labels must be short, printable text.
    pub fn is_valid(&self) -> bool {
        [&self.open, &self.hide, &self.quit].iter().all(|l| {
            let n = l.chars().count();
            n > 0 && n <= MAX_TRAY_LABEL && !l.chars().any(char::is_control)
        })
    }
}

/// The application catalog as Settings / Developer Mode may show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ApplicationCatalogInfo {
    pub status: crate::apps::CatalogStatus,
    /// Names and sources only (never paths). Empty outside developer builds.
    pub applications: Vec<crate::apps::ApplicationSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_labels_must_be_short_printable_text() {
        let ok = TrayLabels {
            open: "Abrir SERSHI".into(),
            hide: "Ocultar SERSHI".into(),
            quit: "Salir de SERSHI".into(),
        };
        assert!(ok.is_valid());
        for bad in ["", "\u{202e}evil\n", &"x".repeat(MAX_TRAY_LABEL + 1)] {
            let labels = TrayLabels {
                open: bad.to_owned(),
                ..ok.clone()
            };
            assert!(!labels.is_valid(), "{bad:?}");
        }
    }
}
