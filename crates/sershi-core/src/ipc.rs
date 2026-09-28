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
