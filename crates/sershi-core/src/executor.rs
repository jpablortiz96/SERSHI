//! The execution pipeline every tool call passes through:
//!
//! ```text
//! ToolCall → registry lookup → policy → (confirmation) → validated tool → result → activity
//! ```

use std::time::Instant;

use serde::Serialize;

use crate::activity::{ActivityKind, ActivityLog, NewActivity};
use crate::ids::ToolId;
use crate::permission::PermissionGrants;
use crate::policy::{ConfirmationReason, DenialReason, PolicyDecision, PolicyEngine};
use crate::tool::{RiskLevel, ToolCall, ToolError, ToolOutput, ToolRegistry};

/// What the user must approve before a call runs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationRequest {
    pub tool_id: ToolId,
    pub tool_name: String,
    pub risk: RiskLevel,
    pub reason: ConfirmationReason,
    pub can_remember: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ExecutionOutcome {
    Completed {
        tool_id: ToolId,
        output: ToolOutput,
        duration_ms: u32,
    },
    ConfirmationRequired {
        request: ConfirmationRequest,
    },
    Denied {
        tool_id: ToolId,
        reason: DenialReason,
    },
    Failed {
        tool_id: ToolId,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct ToolExecutor {
    registry: ToolRegistry,
    policy: PolicyEngine,
}

impl ToolExecutor {
    pub fn new(registry: ToolRegistry, policy: PolicyEngine) -> Self {
        Self { registry, policy }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Runs `call` if policy allows it, recording every step in `activity`.
    ///
    /// Calls that need confirmation are *not* executed; the caller surfaces
    /// the [`ConfirmationRequest`] and re-submits once the user decides.
    pub fn execute(
        &self,
        call: &ToolCall,
        grants: &PermissionGrants,
        activity: &mut ActivityLog,
        now_ms: u64,
    ) -> ExecutionOutcome {
        let tool_id = call.tool_id.clone();
        let Some(tool) = self.registry.get(&tool_id) else {
            activity.record(
                now_ms,
                NewActivity::new(
                    ActivityKind::ToolDenied,
                    "Rejected a request for an unknown tool",
                )
                .tool(&tool_id),
            );
            return ExecutionOutcome::Denied {
                tool_id,
                reason: DenialReason::UnknownTool,
            };
        };
        let definition = tool.definition();
        activity.record(
            now_ms,
            NewActivity::new(
                ActivityKind::ToolRequested,
                format!("Requested {}", definition.name),
            )
            .tool(&tool_id),
        );

        match self.policy.evaluate(definition, call, grants) {
            PolicyDecision::Deny(reason) => {
                activity.record(
                    now_ms,
                    NewActivity::new(
                        ActivityKind::ToolDenied,
                        format!("Blocked {} by policy", definition.name),
                    )
                    .tool(&tool_id),
                );
                ExecutionOutcome::Denied { tool_id, reason }
            }
            PolicyDecision::Confirm {
                reason,
                can_remember,
            } => {
                activity.record(
                    now_ms,
                    NewActivity::new(
                        ActivityKind::ConfirmationRequired,
                        format!("{} is waiting for your approval", definition.name),
                    )
                    .tool(&tool_id),
                );
                ExecutionOutcome::ConfirmationRequired {
                    request: ConfirmationRequest {
                        tool_id,
                        tool_name: definition.name.clone(),
                        risk: definition.risk,
                        reason,
                        can_remember,
                    },
                }
            }
            PolicyDecision::Allow => {
                let started = Instant::now();
                let result = tool.execute(&call.input);
                let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
                self.finish(
                    tool_id,
                    &definition.name,
                    result,
                    duration_ms,
                    activity,
                    now_ms,
                )
            }
        }
    }

    fn finish(
        &self,
        tool_id: ToolId,
        name: &str,
        result: Result<ToolOutput, ToolError>,
        duration_ms: u32,
        activity: &mut ActivityLog,
        now_ms: u64,
    ) -> ExecutionOutcome {
        match result {
            Ok(output) => {
                activity.record(
                    now_ms,
                    NewActivity::new(ActivityKind::ToolCompleted, format!("{name} completed"))
                        .tool(&tool_id)
                        .duration(duration_ms),
                );
                ExecutionOutcome::Completed {
                    tool_id,
                    output,
                    duration_ms,
                }
            }
            Err(error) => {
                // The error text may echo input; keep it out of the activity log.
                activity.record(
                    now_ms,
                    NewActivity::new(ActivityKind::ToolFailed, format!("{name} failed"))
                        .tool(&tool_id)
                        .duration(duration_ms),
                );
                ExecutionOutcome::Failed {
                    tool_id,
                    message: error.to_string(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::Value;

    use super::*;
    use crate::permission::PermissionState;
    use crate::platform::Platform;
    use crate::tool::test_support::FakeTool;
    use crate::tool::{CallOrigin, Tool};

    fn executor_with(tools: Vec<FakeTool>) -> ToolExecutor {
        let mut registry = ToolRegistry::default();
        for t in tools {
            registry.register(Arc::new(t)).unwrap();
        }
        ToolExecutor::new(registry, PolicyEngine::new(Platform::Linux))
    }

    fn call(id: &str) -> ToolCall {
        ToolCall::new(ToolId::new(id).unwrap(), Value::Null, CallOrigin::Agent)
    }

    fn kinds(log: &ActivityLog) -> Vec<ActivityKind> {
        log.recent(10).into_iter().rev().map(|e| e.kind).collect()
    }

    #[test]
    fn allowed_call_runs_and_is_audited() {
        let exec = executor_with(vec![FakeTool::new(
            "system.get_info",
            RiskLevel::Safe,
            &["system.info.read"],
        )]);
        let mut log = ActivityLog::default();
        let out = exec.execute(
            &call("system.get_info"),
            &PermissionGrants::default(),
            &mut log,
            1,
        );
        assert!(matches!(out, ExecutionOutcome::Completed { .. }));
        assert_eq!(
            kinds(&log),
            [ActivityKind::ToolRequested, ActivityKind::ToolCompleted]
        );
    }

    #[test]
    fn unknown_tools_are_denied_not_guessed() {
        let exec = executor_with(vec![]);
        let mut log = ActivityLog::default();
        let out = exec.execute(
            &call("shell.run"),
            &PermissionGrants::default(),
            &mut log,
            1,
        );
        assert!(matches!(
            out,
            ExecutionOutcome::Denied {
                reason: DenialReason::UnknownTool,
                ..
            }
        ));
    }

    /// A tool whose execution would be observable if policy were bypassed.
    #[derive(Debug)]
    struct Tripwire(FakeTool, std::sync::atomic::AtomicBool);
    impl Tool for Tripwire {
        fn definition(&self) -> &crate::tool::ToolDefinition {
            self.0.definition()
        }
        fn execute(&self, input: &Value) -> Result<ToolOutput, ToolError> {
            self.1.store(true, std::sync::atomic::Ordering::SeqCst);
            self.0.execute(input)
        }
    }

    #[test]
    fn confirmation_and_denial_never_execute_the_tool() {
        let high = Arc::new(Tripwire(
            FakeTool::new("files.delete", RiskLevel::HighRisk, &["files.delete"]),
            Default::default(),
        ));
        let denied = Arc::new(Tripwire(
            FakeTool::new("files.move", RiskLevel::Sensitive, &["files.write"]),
            Default::default(),
        ));
        let mut registry = ToolRegistry::default();
        registry.register(high.clone()).unwrap();
        registry.register(denied.clone()).unwrap();
        let exec = ToolExecutor::new(registry, PolicyEngine::new(Platform::Linux));

        let mut grants = PermissionGrants::empty();
        grants.set(
            crate::ids::PermissionId::new("files.delete").unwrap(),
            PermissionState::Granted,
        );
        grants.set(
            crate::ids::PermissionId::new("files.write").unwrap(),
            PermissionState::Denied,
        );
        let mut log = ActivityLog::default();

        let out = exec.execute(&call("files.delete"), &grants, &mut log, 1);
        assert!(matches!(out, ExecutionOutcome::ConfirmationRequired { .. }));
        let out = exec.execute(&call("files.move"), &grants, &mut log, 1);
        assert!(matches!(out, ExecutionOutcome::Denied { .. }));

        use std::sync::atomic::Ordering::SeqCst;
        assert!(
            !high.1.load(SeqCst),
            "high-risk tool ran without confirmation"
        );
        assert!(!denied.1.load(SeqCst), "denied tool ran");
    }

    #[test]
    fn failures_are_reported_without_leaking_details_into_activity() {
        let mut tool = FakeTool::new("system.get_info", RiskLevel::Safe, &["system.info.read"]);
        tool.result = Err(ToolError::Failed("C:\\Users\\alice\\secret.txt".into()));
        let exec = executor_with(vec![tool]);
        let mut log = ActivityLog::default();
        let out = exec.execute(
            &call("system.get_info"),
            &PermissionGrants::default(),
            &mut log,
            1,
        );
        assert!(matches!(out, ExecutionOutcome::Failed { .. }));
        assert!(log.recent(10).iter().all(|e| !e.summary.contains("alice")));
    }
}
