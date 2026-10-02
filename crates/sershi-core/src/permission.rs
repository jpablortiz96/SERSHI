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

/// Permissions granted on first run: read-only system information and
/// launching installed applications (a safe, user-requested action). Closing
/// applications and everything else start at [`PermissionState::Ask`].
pub const DEFAULT_GRANTED: &[&str] = &["system.info.read", "system.apps.launch"];

/// The permissions a user may configure in Settings › Security (Gate 4.1).
///
/// A closed list on purpose: only low-risk, implemented capabilities appear
/// here. There is no "allow everything", no way to name an arbitrary tool or
/// permission, and nothing high risk can ever be added — a high-risk tool
/// always needs the trusted confirmation whatever is granted (see
/// [`crate::policy`]), and the service refuses to configure a permission
/// that a high-risk tool uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ConfigurablePermission {
    /// Open installed applications (`system.apps.launch`).
    OpenApplications,
    /// Close running applications (`system.apps.close`).
    CloseApplications,
    /// Read memory, processor and system information (`system.info.read`).
    SystemInformation,
}

impl ConfigurablePermission {
    pub const ALL: [Self; 3] = [
        Self::OpenApplications,
        Self::CloseApplications,
        Self::SystemInformation,
    ];

    /// The permission this setting controls.
    pub fn permission(self) -> &'static str {
        match self {
            Self::OpenApplications => "system.apps.launch",
            Self::CloseApplications => "system.apps.close",
            Self::SystemInformation => "system.info.read",
        }
    }

    pub fn permission_id(self) -> PermissionId {
        // The three ids above are valid by construction (tested below).
        PermissionId::new(self.permission()).unwrap_or_else(|_| unreachable!())
    }

    /// SERSHI's default for a new user (unchanged since Gate 1A).
    pub fn default_setting(self) -> PermissionSetting {
        if DEFAULT_GRANTED.contains(&self.permission()) {
            PermissionSetting::AlwaysAllow
        } else {
            PermissionSetting::AskEveryTime
        }
    }
}

/// What the user chose for a [`ConfigurablePermission`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum PermissionSetting {
    /// Run without an extra confirmation when the user asks (typed or
    /// spoken). Actions proposed by a language model still need the trusted
    /// confirmation for sensitive tools.
    AlwaysAllow,
    /// Ask in the trusted confirmation window every time.
    AskEveryTime,
}

impl PermissionSetting {
    pub fn state(self) -> PermissionState {
        match self {
            Self::AlwaysAllow => PermissionState::Granted,
            Self::AskEveryTime => PermissionState::Ask,
        }
    }

    /// Whether changing from `current` to `self` grants more authority (and
    /// so must be approved in the trusted confirmation window).
    pub fn loosens(self, current: Self) -> bool {
        self == Self::AlwaysAllow && current == Self::AskEveryTime
    }
}

/// One configurable permission as Settings shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    pub permission: ConfigurablePermission,
    pub setting: PermissionSetting,
    pub default_setting: PermissionSetting,
}

/// A user's stored close setting for one application (Gate 4.1.1),
/// e.g. "Outlook: Always allow". Applies to the user's own requests only;
/// a close proposed by a language model always asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AppPermission {
    /// The trusted catalog id (never a path).
    pub app_id: String,
    /// Name as Windows presents it (for display).
    pub display_name: String,
    pub close: PermissionSetting,
}

/// Most per-application settings kept.
pub const MAX_APP_PERMISSIONS: usize = 32;

/// A catalog id: lowercase letters, digits, `.`, `-`, `+`; bounded.
pub fn valid_app_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '+'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionGrants {
    decisions: BTreeMap<PermissionId, PermissionState>,
    /// Per-application close settings, by catalog id.
    apps: BTreeMap<String, AppPermission>,
}

impl Default for PermissionGrants {
    fn default() -> Self {
        let decisions = DEFAULT_GRANTED
            .iter()
            .filter_map(|p| PermissionId::new(*p).ok())
            .map(|p| (p, PermissionState::Granted))
            .collect();
        Self {
            decisions,
            apps: BTreeMap::new(),
        }
    }
}

impl PermissionGrants {
    /// Grants with nothing pre-approved.
    pub fn empty() -> Self {
        Self {
            decisions: BTreeMap::new(),
            apps: BTreeMap::new(),
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

    /// The current setting of a configurable permission.
    pub fn setting(&self, permission: ConfigurablePermission) -> PermissionSetting {
        match self.state(&permission.permission_id()) {
            PermissionState::Granted => PermissionSetting::AlwaysAllow,
            PermissionState::Ask | PermissionState::Denied => PermissionSetting::AskEveryTime,
        }
    }

    pub fn configure(&mut self, permission: ConfigurablePermission, setting: PermissionSetting) {
        self.set(permission.permission_id(), setting.state());
    }

    /// The user's stored close setting for one application, if any.
    pub fn app_close(&self, app_id: &str) -> Option<PermissionSetting> {
        self.apps.get(app_id).map(|a| a.close)
    }

    /// Stores a per-application close setting. Refused (false) for an
    /// invalid id or name, or when the bounded list is full.
    pub fn set_app_close(
        &mut self,
        app_id: &str,
        display_name: &str,
        setting: PermissionSetting,
    ) -> bool {
        let name: String = display_name.trim().chars().take(128).collect();
        if !valid_app_id(app_id) || name.is_empty() {
            return false;
        }
        if !self.apps.contains_key(app_id) && self.apps.len() >= MAX_APP_PERMISSIONS {
            return false;
        }
        self.apps.insert(
            app_id.to_owned(),
            AppPermission {
                app_id: app_id.to_owned(),
                display_name: name,
                close: setting,
            },
        );
        true
    }

    /// Per-application settings, by id.
    pub fn app_permissions(&self) -> Vec<AppPermission> {
        self.apps.values().cloned().collect()
    }

    /// Whether `permission` is granted because the user chose so (it is not
    /// granted by default).
    pub fn granted_by_user(&self, permission: &PermissionId) -> bool {
        self.state(permission) == PermissionState::Granted
            && !DEFAULT_GRANTED.contains(&permission.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_read_only_and_launch_permissions_are_granted_by_default() {
        let grants = PermissionGrants::default();
        let info = PermissionId::new("system.info.read").unwrap();
        assert_eq!(grants.state(&info), PermissionState::Granted);
        let launch = PermissionId::new("system.apps.launch").unwrap();
        assert_eq!(grants.state(&launch), PermissionState::Granted);
        let close = PermissionId::new("system.apps.close").unwrap();
        assert_eq!(grants.state(&close), PermissionState::Ask);
    }

    #[test]
    fn configurable_permissions_keep_the_gate_1a_defaults() {
        let grants = PermissionGrants::default();
        for permission in ConfigurablePermission::ALL {
            assert_eq!(grants.setting(permission), permission.default_setting());
            assert!(!grants.granted_by_user(&permission.permission_id()));
        }
        assert_eq!(
            ConfigurablePermission::CloseApplications.default_setting(),
            PermissionSetting::AskEveryTime
        );
        assert_eq!(
            ConfigurablePermission::OpenApplications.default_setting(),
            PermissionSetting::AlwaysAllow
        );
    }

    #[test]
    fn only_the_listed_permissions_can_be_configured() {
        // The settings format names permissions by a closed enum: an
        // unknown tool, a raw permission id or "everything" do not parse.
        for raw in [
            "\"allowAll\"",
            "\"alwaysAllowAllActions\"",
            "\"files.delete\"",
            "\"system.apps.close\"",
            "\"*\"",
        ] {
            assert!(
                serde_json::from_str::<ConfigurablePermission>(raw).is_err(),
                "{raw}"
            );
        }
        assert!(serde_json::from_str::<PermissionSetting>("\"granted\"").is_err());
    }

    #[test]
    fn per_application_settings_are_bounded_and_validated() {
        let mut grants = PermissionGrants::default();
        assert!(grants.set_app_close("outlook", "Outlook", PermissionSetting::AlwaysAllow));
        assert_eq!(
            grants.app_close("outlook"),
            Some(PermissionSetting::AlwaysAllow)
        );
        assert_eq!(grants.app_close("excel"), None);
        for bad in ["", r"C:\x.exe", "Outlook", "../x", "a b", &"a".repeat(97)] {
            assert!(
                !grants.set_app_close(bad, "X", PermissionSetting::AlwaysAllow),
                "{bad}"
            );
        }
        for i in 0..MAX_APP_PERMISSIONS {
            grants.set_app_close(&format!("app-{i}"), "App", PermissionSetting::AskEveryTime);
        }
        assert!(!grants.set_app_close("one-more", "App", PermissionSetting::AlwaysAllow));
        // An existing entry can still change.
        assert!(grants.set_app_close("outlook", "Outlook", PermissionSetting::AskEveryTime));
        assert!(grants.app_permissions().len() <= MAX_APP_PERMISSIONS);
    }

    #[test]
    fn a_user_grant_is_told_apart_from_a_default_one() {
        let mut grants = PermissionGrants::default();
        grants.configure(
            ConfigurablePermission::CloseApplications,
            PermissionSetting::AlwaysAllow,
        );
        let close = ConfigurablePermission::CloseApplications.permission_id();
        assert!(grants.granted_by_user(&close));
        let launch = ConfigurablePermission::OpenApplications.permission_id();
        assert!(!grants.granted_by_user(&launch));
        assert!(PermissionSetting::AlwaysAllow.loosens(PermissionSetting::AskEveryTime));
        assert!(!PermissionSetting::AskEveryTime.loosens(PermissionSetting::AlwaysAllow));
    }
}
