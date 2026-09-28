//! User permission grants.
//!
//! A permission (e.g. `system.info.read`) is a named capability a tool needs.
//! The user's decision for each permission is one of granted / ask / denied.
//! Grants are data the user can inspect and change; they are persisted by the
//! storage layer (v0.1 milestone) and never inferred by a model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::PermissionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum PermissionState {
    /// Allowed without asking (subject to the tool's risk level).
    Granted,
    /// Ask the user each time.
    Ask,
    /// Never allowed.
    Denied,
}

/// Permissions granted on first run. Only harmless, read-only system
/// information is pre-approved; everything else starts at [`PermissionState::Ask`].
pub const DEFAULT_GRANTED: &[&str] = &["system.info.read"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionGrants {
    decisions: BTreeMap<PermissionId, PermissionState>,
}

impl Default for PermissionGrants {
    fn default() -> Self {
        let decisions = DEFAULT_GRANTED
            .iter()
            .filter_map(|p| PermissionId::new(*p).ok())
            .map(|p| (p, PermissionState::Granted))
            .collect();
        Self { decisions }
    }
}

impl PermissionGrants {
    /// Grants with nothing pre-approved.
    pub fn empty() -> Self {
        Self {
            decisions: BTreeMap::new(),
        }
    }

    /// Unknown permissions default to [`PermissionState::Ask`].
    pub fn state(&self, permission: &PermissionId) -> PermissionState {
        self.decisions
            .get(permission)
            .copied()
            .unwrap_or(PermissionState::Ask)
    }

    pub fn set(&mut self, permission: PermissionId, state: PermissionState) {
        self.decisions.insert(permission, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_read_only_system_info_is_granted_by_default() {
        let grants = PermissionGrants::default();
        let info = PermissionId::new("system.info.read").unwrap();
        assert_eq!(grants.state(&info), PermissionState::Granted);
        let launch = PermissionId::new("system.apps.launch").unwrap();
        assert_eq!(grants.state(&launch), PermissionState::Ask);
    }
}
