//! The policy engine: decides whether a tool call may run.
//!
//! Inputs are the tool's *registered* definition (risk, permissions,
//! platforms), the user's grants and the call's origin. The call itself only
//! names a tool and carries input; it cannot influence its own risk.
//!
//! Decision order (first match wins):
//! 1. Prohibited tool                  → deny
//! 2. Not supported on this platform   → deny
//! 3. Any required permission denied   → deny
//! 4. High risk                        → confirm (never rememberable)
//! 5. Any permission still "ask"       → confirm
//! 6. Sensitive and not user-initiated → confirm
//! 7. Otherwise                        → allow

use std::collections::BTreeSet;

use serde::Serialize;

use crate::ids::{PermissionId, ToolId};
use crate::permission::{PermissionGrants, PermissionState};
use crate::platform::Platform;
use crate::tool::{CallOrigin, RiskLevel, ToolCall, ToolDefinition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum DenialReason {
    UnknownTool,
    Prohibited,
    UnsupportedPlatform,
    PermissionDenied { permission: PermissionId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationReason {
    HighRisk,
    PermissionUndecided,
    AgentInitiatedSensitiveAction,
    /// The user asked in Settings to stop being asked for a permission
    /// ("Always allow"): granting more authority is itself approved in the
    /// trusted window (Gate 4.1).
    PermissionChange,
}

/// Why an action that ran was allowed (audit and the action ledger, Gate
/// 4.1). Recorded so that "SERSHI closed Outlook hands-free" can always be
/// traced to a stored setting the user chose in Settings — never to the
/// words that asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Authorization {
    /// SERSHI's default policy allows it (a safe action whose permission is
    /// granted by default).
    Policy,
    /// Allowed because the user set the permission to "Always allow" in
    /// Settings › Security (a stored, revocable decision).
    StoredPermission,
    /// Approved by the user in the trusted confirmation window.
    TrustedConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Confirm {
        reason: ConfirmationReason,
        /// Whether "remember this decision" may be offered. Never true for
        /// high-risk actions.
        can_remember: bool,
    },
    Deny(DenialReason),
}

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    platform: Platform,
    prohibited: BTreeSet<ToolId>,
}

impl PolicyEngine {
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            prohibited: BTreeSet::new(),
        }
    }

    /// Permanently block a tool regardless of grants (e.g. by enterprise
    /// policy or because a skill was revoked).
    pub fn prohibit(&mut self, tool: ToolId) {
        self.prohibited.insert(tool);
    }

    pub fn evaluate(
        &self,
        definition: &ToolDefinition,
        call: &ToolCall,
        grants: &PermissionGrants,
    ) -> PolicyDecision {
        if self.prohibited.contains(&definition.id) {
            return PolicyDecision::Deny(DenialReason::Prohibited);
        }
        if !definition.platforms.contains(&self.platform) {
            return PolicyDecision::Deny(DenialReason::UnsupportedPlatform);
        }

        let mut undecided = false;
        for permission in &definition.permissions {
            match grants.state(permission) {
                PermissionState::Denied => {
                    return PolicyDecision::Deny(DenialReason::PermissionDenied {
                        permission: permission.clone(),
                    });
                }
                PermissionState::Ask => undecided = true,
                PermissionState::Granted => {}
            }
        }

        match definition.risk {
            RiskLevel::HighRisk => PolicyDecision::Confirm {
                reason: ConfirmationReason::HighRisk,
                can_remember: false,
            },
            _ if undecided => PolicyDecision::Confirm {
                reason: ConfirmationReason::PermissionUndecided,
                can_remember: true,
            },
            RiskLevel::Sensitive if call.origin != CallOrigin::User => PolicyDecision::Confirm {
                reason: ConfirmationReason::AgentInitiatedSensitiveAction,
                can_remember: false,
            },
            RiskLevel::Safe | RiskLevel::Sensitive => PolicyDecision::Allow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;
    use crate::tool::test_support::FakeTool;
    use serde_json::Value;

    fn call(tool: &FakeTool, origin: CallOrigin) -> ToolCall {
        ToolCall::new(tool.definition().id.clone(), Value::Null, origin)
    }

    fn granted(perms: &[&str]) -> PermissionGrants {
        let mut g = PermissionGrants::empty();
        for p in perms {
            g.set(PermissionId::new(*p).unwrap(), PermissionState::Granted);
        }
        g
    }

    fn engine() -> PolicyEngine {
        PolicyEngine::new(Platform::Linux)
    }

    #[test]
    fn safe_tool_with_granted_permission_is_allowed() {
        let tool = FakeTool::new("system.get_info", RiskLevel::Safe, &["system.info.read"]);
        let decision = engine().evaluate(
            tool.definition(),
            &call(&tool, CallOrigin::Agent),
            &granted(&["system.info.read"]),
        );
        assert_eq!(decision, PolicyDecision::Allow);
    }

    #[test]
    fn undecided_permission_requires_confirmation_even_for_safe_tools() {
        let tool = FakeTool::new("system.open_app", RiskLevel::Safe, &["system.apps.launch"]);
        let decision = engine().evaluate(
            tool.definition(),
            &call(&tool, CallOrigin::User),
            &PermissionGrants::empty(),
        );
        assert_eq!(
            decision,
            PolicyDecision::Confirm {
                reason: ConfirmationReason::PermissionUndecided,
                can_remember: true
            }
        );
    }

    #[test]
    fn denied_permission_wins_over_everything_else() {
        let tool = FakeTool::new(
            "files.move",
            RiskLevel::Sensitive,
            &["files.read", "files.write"],
        );
        let mut grants = granted(&["files.read"]);
        grants.set(
            PermissionId::new("files.write").unwrap(),
            PermissionState::Denied,
        );
        let decision =
            engine().evaluate(tool.definition(), &call(&tool, CallOrigin::User), &grants);
        assert!(matches!(
            decision,
            PolicyDecision::Deny(DenialReason::PermissionDenied { ref permission })
                if permission.as_str() == "files.write"
        ));
    }

    #[test]
    fn high_risk_always_confirms_and_cannot_be_remembered() {
        let tool = FakeTool::new("files.delete", RiskLevel::HighRisk, &["files.delete"]);
        for origin in [CallOrigin::User, CallOrigin::Agent, CallOrigin::Routine] {
            let decision = engine().evaluate(
                tool.definition(),
                &call(&tool, origin),
                &granted(&["files.delete"]),
            );
            assert_eq!(
                decision,
                PolicyDecision::Confirm {
                    reason: ConfirmationReason::HighRisk,
                    can_remember: false
                },
                "origin {origin:?}"
            );
        }
    }

    #[test]
    fn model_proposed_sensitive_actions_need_confirmation_even_when_granted() {
        let tool = FakeTool::new("email.send", RiskLevel::Sensitive, &["email.send"]);
        let grants = granted(&["email.send"]);
        let agent = engine().evaluate(tool.definition(), &call(&tool, CallOrigin::Agent), &grants);
        assert!(matches!(
            agent,
            PolicyDecision::Confirm {
                reason: ConfirmationReason::AgentInitiatedSensitiveAction,
                ..
            }
        ));
        let user = engine().evaluate(tool.definition(), &call(&tool, CallOrigin::User), &grants);
        assert_eq!(user, PolicyDecision::Allow);
    }

    #[test]
    fn prohibited_tools_are_denied_even_with_grants() {
        let tool = FakeTool::new("system.get_info", RiskLevel::Safe, &["system.info.read"]);
        let mut policy = engine();
        policy.prohibit(tool.definition().id.clone());
        let decision = policy.evaluate(
            tool.definition(),
            &call(&tool, CallOrigin::User),
            &granted(&["system.info.read"]),
        );
        assert_eq!(decision, PolicyDecision::Deny(DenialReason::Prohibited));
    }

    #[test]
    fn tools_for_other_platforms_are_denied() {
        let mut tool = FakeTool::new("system.get_info", RiskLevel::Safe, &["system.info.read"]);
        tool.def.platforms = vec![Platform::Windows];
        let decision = engine().evaluate(
            tool.definition(),
            &call(&tool, CallOrigin::User),
            &granted(&["system.info.read"]),
        );
        assert_eq!(
            decision,
            PolicyDecision::Deny(DenialReason::UnsupportedPlatform)
        );
    }
}
