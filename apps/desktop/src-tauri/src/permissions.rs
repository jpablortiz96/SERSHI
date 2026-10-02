//! Stored permission settings (Gate 4.1): Settings › Security.
//!
//! Rust-authoritative. The file (`permissions.json` in the per-user app
//! config directory) holds only the closed list of configurable permissions
//! (`ConfigurablePermission`) and their setting; an unknown permission, an
//! unknown value or any other field makes the whole file invalid, and
//! SERSHI falls back to its defaults (fail safe: "Ask every time" for
//! closing applications). Nothing else — no tool ids, no "allow
//! everything" — can be stored.
//!
//! Who can change it:
//! - more restrictive ("Ask every time"): Settings, applied at once;
//! - less restrictive ("Always allow"): Settings asks, and the change waits
//!   in the trusted confirmation window like a sensitive action. Voice, the
//!   Agent Brain and the Command Center cannot apply it themselves.
//!
//! Every change is audited (`PermissionChanged`).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::permission::{ConfigurablePermission, PermissionSetting, PermissionStatus};
use sershi_core::service::{
    CommandOutcome, OutcomeDetail, PermissionChange, PermissionChangeError,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::confirmation;
use crate::runtime::{Runtime, broadcast};
use crate::surfaces::MAIN;

/// Current permission settings (`PermissionStatus[]`), Command Center only.
pub const PERMISSIONS_EVENT: &str = "sershi://permissions";

const FILE: &str = "permissions.json";
const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    version: u32,
    permissions: BTreeMap<ConfigurablePermission, PermissionSetting>,
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(FILE))
}

/// The user's stored settings; empty (defaults) if none or invalid.
pub fn load(app: &AppHandle) -> Vec<(ConfigurablePermission, PermissionSetting)> {
    let Some(path) = path(app) else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Stored>(&raw) {
        Ok(stored) if stored.version == VERSION => stored.permissions.into_iter().collect(),
        _ => {
            eprintln!("SERSHI: stored permission settings are invalid; using defaults");
            Vec::new()
        }
    }
}

/// Writes the current settings (atomically: a temporary file, then rename).
fn save(app: &AppHandle, settings: &[PermissionStatus]) {
    let Some(path) = path(app) else {
        return;
    };
    let stored = Stored {
        version: VERSION,
        permissions: settings.iter().map(|s| (s.permission, s.setting)).collect(),
    };
    let Ok(json) = serde_json::to_string_pretty(&stored) else {
        return;
    };
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| {
            let tmp = path.with_extension("json.tmp");
            fs::write(&tmp, json)?;
            fs::rename(&tmp, &path)
        });
    if written.is_err() {
        eprintln!("SERSHI: permission settings could not be saved");
    }
}

pub fn current(app: &AppHandle) -> Result<Vec<PermissionStatus>, IpcError> {
    app.state::<Runtime>()
        .with_service(|s| s.permission_settings())
}

/// Saves and announces the settings after a change.
fn publish(app: &AppHandle) {
    if let Ok(settings) = current(app) {
        save(app, &settings);
        let _ = app.emit_to(MAIN, PERMISSIONS_EVENT, &settings);
    }
}

/// Settings asked to change a permission.
pub fn request_change(
    app: &AppHandle,
    permission: ConfigurablePermission,
    setting: PermissionSetting,
) -> Result<PermissionChange, IpcError> {
    let change = app
        .state::<Runtime>()
        .with_service(|s| {
            s.request_permission_change(permission, setting, &mut |e| broadcast(app, e))
        })?
        .map_err(|error| match error {
            PermissionChangeError::Busy => IpcError::new(
                IpcErrorCode::Unavailable,
                "SERSHI is busy. Try again in a moment.",
            ),
            PermissionChangeError::NotConfigurable => IpcError::new(
                IpcErrorCode::NotAllowed,
                "This permission can't be changed.",
            ),
            PermissionChangeError::Unavailable => IpcError::new(
                IpcErrorCode::Unavailable,
                "The confirmation window is unavailable.",
            ),
        })?;
    match change {
        PermissionChange::Applied => publish(app),
        // The trusted window opens; only its decision applies the change.
        PermissionChange::NeedsConfirmation => confirmation::sync(app),
        PermissionChange::Unchanged => {}
    }
    Ok(change)
}

/// Whether an outcome is a permission decision (it belongs to Settings, not
/// to the conversation). Publishes the settings if so.
pub fn handled(app: &AppHandle, outcome: &CommandOutcome) -> bool {
    match outcome.detail {
        Some(OutcomeDetail::Permission { applied, .. }) => {
            if applied {
                publish(app);
            } else if let Ok(settings) = current(app) {
                let _ = app.emit_to(MAIN, PERMISSIONS_EVENT, &settings);
            }
            true
        }
        _ => false,
    }
}
