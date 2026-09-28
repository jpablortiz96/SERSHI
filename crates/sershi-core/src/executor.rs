//! The execution pipeline every tool call passes through:
//!
//! ```text
//! ToolCall → registry → policy → prepare (resolve target) ─┬─ allow   → execute → audit
//!                                                            └─ confirm → ConfirmationDraft
//! ```
//!
//! A call that needs confirmation is never executed here. The service stores
//! it (see `confirmation.rs`) and, once the user approves, calls
//! [`ToolExecutor::execute_approved`], which re-checks policy and verifies
//! the target is still the one the user approved.

use std::sync::Arc;
use std::time::Instant;

use crate::activity::{ActivityKind, ActivityLog, NewActivity};
use crate::confirmation::{ConfirmationDraft, ConfirmationSubject};
use crate::ids::ToolId;
use crate::permission::PermissionGrants;
use crate::policy::{DenialReason, PolicyDecision, PolicyEngine};
use crate::tool::{Prepared, Severity, Tool, ToolCall, ToolError, ToolOutput, ToolRegistry};

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionOutcome {
    Completed {
        tool_id: ToolId,
        output: ToolOutput,
        duration_ms: u32,
    },
    /// Policy requires approval; nothing ran.
    ConfirmationRequired(ConfirmationDraft),
    Denied {
        tool_id: ToolId,
        reason: DenialReason,
    },
    /// Unexpected failure. `message` is internal detail, never shown raw.
    Failed { tool_id: ToolId, message: String },
    /// A structured negative result (not found, ambiguous, blocked…).
    Declined {
        tool_id: ToolId,
        output: ToolOutput,
        severity: Severity,
    },
    /// On approval, the target no longer matches what the user approved.
    SubjectChanged { tool_id: ToolId },
}

struct Planned {
    tool: Arc<dyn Tool>,
    decision: PolicyDecision,
    prepared: Prepared,
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
    /// `on_execute` is invoked just before the tool acts (after planning),
    /// so the caller can reflect the Executing state.
    pub fn execute(
        &self,
        call: &ToolCall,
        grants: &PermissionGrants,
        activity: &mut ActivityLog,
        now_ms: u64,
        on_execute: &mut dyn FnMut(),
    ) -> ExecutionOutcome {
        let planned = match self.plan(call, grants, activity, now_ms, true) {
            Ok(planned) => planned,
            Err(outcome) => return *outcome,
        };
        match planned.decision {
            PolicyDecision::Confirm { reason, .. } => {
                let definition = planned.tool.definition();
                activity.record(
                    now_ms,
                    NewActivity::new(
                        ActivityKind::ConfirmationRequired,
                        format!("{} is waiting for your approval", definition.name),
                    )
                    .tool(&call.tool_id)
                    .subject_opt(subject_name(&planned.prepared.subject)),
                );
                ExecutionOutcome::ConfirmationRequired(ConfirmationDraft {
                    call: call.clone(),
                    action: planned.prepared.action,
                    subject: planned.prepared.subject,
                    risk: definition.risk,
                    reason,
                })
            }
            // `plan` already returned denials.
            PolicyDecision::Allow | PolicyDecision::Deny(_) => {
                on_execute();
                self.run(&planned.tool, call, activity, now_ms)
            }
        }
    }

    /// Runs a call the user approved. Policy is evaluated again (a revoked
    /// permission still wins) and the resolved target must equal the one
    /// shown in the confirmation.
    pub fn execute_approved(
        &self,
        call: &ToolCall,
        approved_subject: &Option<ConfirmationSubject>,
        grants: &PermissionGrants,
        activity: &mut ActivityLog,
        now_ms: u64,
    ) -> ExecutionOutcome {
        let planned = match self.plan(call, grants, activity, now_ms, false) {
            Ok(planned) => planned,
            Err(outcome) => return *outcome,
        };
        if &planned.prepared.subject != approved_subject {
            activity.record(
                now_ms,
                NewActivity::new(
                    ActivityKind::ToolDenied,
                    "Stopped an approved action whose target changed",
                )
                .tool(&call.tool_id),
            );
            return ExecutionOutcome::SubjectChanged {
                tool_id: call.tool_id.clone(),
            };
        }
        self.run(&planned.tool, call, activity, now_ms)
    }

    /// Registry lookup, policy, then `prepare`. Returns the terminal outcome
    /// when the call cannot proceed.
    fn plan(
        &self,
        call: &ToolCall,
        grants: &PermissionGrants,
        activity: &mut ActivityLog,
        now_ms: u64,
        record_request: bool,
    ) -> Result<Planned, Box<ExecutionOutcome>> {
        let tool_id = call.tool_id.clone();
        let Some(tool) = self.registry.get(&tool_id).cloned() else {
            activity.record(
                now_ms,
                NewActivity::new(
                    ActivityKind::ToolDenied,
                    "Rejected a request for an unknown tool",
                )
                .tool(&tool_id),
            );
            return Err(Box::new(ExecutionOutcome::Denied {
                tool_id,
                reason: DenialReason::UnknownTool,
            }));
        };
        let name = tool.definition().name.clone();
        if record_request {
            activity.record(
                now_ms,
                NewActivity::new(ActivityKind::ToolRequested, format!("Requested {name}"))
                    .tool(&tool_id),
            );
        }
        let decision = self.policy.evaluate(tool.definition(), call, grants);
        if let PolicyDecision::Deny(reason) = decision {
            activity.record(
                now_ms,
                NewActivity::new(
                    ActivityKind::ToolDenied,
                    format!("Blocked {name} by policy"),
                )
                .tool(&tool_id),
            );
            return Err(Box::new(ExecutionOutcome::Denied { tool_id, reason }));
        }
        match tool.prepare(&call.input) {
            Ok(prepared) => Ok(Planned {
                tool,
                decision,
                prepared,
            }),
            Err(error) => Err(Box::new(finish(
                tool_id,
                &name,
                Err(error),
                0,
                activity,
                now_ms,
            ))),
        }
    }

    fn run(
        &self,
        tool: &Arc<dyn Tool>,
        call: &ToolCall,
        activity: &mut ActivityLog,
        now_ms: u64,
    ) -> ExecutionOutcome {
        let started = Instant::now();
        let result = tool.execute(&call.input);
        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let name = tool.definition().name.clone();
        finish(
            call.tool_id.clone(),
            &name,
            result,
            duration_ms,
            activity,
            now_ms,
        )
    }
}

fn subject_name(subject: &Option<ConfirmationSubject>) -> Option<String> {
    subject.as_ref().map(|s| match s {
        ConfirmationSubject::Application { application } => application.display_name.clone(),
    })
}

fn finish(
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
                    .subject_opt(output.subject.clone())
                    .duration(duration_ms),
            );
            ExecutionOutcome::Completed {
                tool_id,
                output,
                duration_ms,
            }
        }
        Err(ToolError::Declined { output, severity }) => {
            let kind = match severity {
                Severity::Attention => ActivityKind::ToolDeclined,
                Severity::Failure => ActivityKind::ToolFailed,
            };
            // Summary is a fixed template: the output may echo user input.
            activity.record(
                now_ms,
                NewActivity::new(kind, format!("{name} could not complete"))
                    .tool(&tool_id)
                    .subject_opt(output.subject.clone())
                    .duration(duration_ms),
            );
            ExecutionOutcome::Declined {
                tool_id,
                output,
                severity,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::Value;

    use super::*;
    use crate::permission::PermissionState;
    use crate::platform::Platform;
    use crate::tool::RiskLevel;
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
            &mut || {},
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
            &mut || {},
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

        let out = exec.execute(&call("files.delete"), &grants, &mut log, 1, &mut || {});
        assert!(matches!(out, ExecutionOutcome::ConfirmationRequired { .. }));
        let out = exec.execute(&call("files.move"), &grants, &mut log, 1, &mut || {});
        assert!(matches!(out, ExecutionOutcome::Denied { .. }));

        use std::sync::atomic::Ordering::SeqCst;
        assert!(
            !high.1.load(SeqCst),
            "high-risk tool ran without confirmation"
        );
        assert!(!denied.1.load(SeqCst), "denied tool ran");
    }

    fn granted(perms: &[&str]) -> PermissionGrants {
        let mut g = PermissionGrants::empty();
        for p in perms {
            g.set(
                crate::ids::PermissionId::new(*p).unwrap(),
                PermissionState::Granted,
            );
        }
        g
    }

    #[test]
    fn approved_calls_rerun_policy_so_revocation_still_wins() {
        let tool = Arc::new(Tripwire(
            FakeTool::new("files.delete", RiskLevel::HighRisk, &["files.delete"]),
            Default::default(),
        ));
        let mut registry = ToolRegistry::default();
        registry.register(tool.clone()).unwrap();
        let exec = ToolExecutor::new(registry, PolicyEngine::new(Platform::Linux));
        let mut log = ActivityLog::default();

        let mut revoked = PermissionGrants::empty();
        revoked.set(
            crate::ids::PermissionId::new("files.delete").unwrap(),
            PermissionState::Denied,
        );
        let out = exec.execute_approved(&call("files.delete"), &None, &revoked, &mut log, 1);
        assert!(matches!(out, ExecutionOutcome::Denied { .. }));
        assert!(!tool.1.load(std::sync::atomic::Ordering::SeqCst));

        let out = exec.execute_approved(
            &call("files.delete"),
            &None,
            &granted(&["files.delete"]),
            &mut log,
            1,
        );
        assert!(matches!(out, ExecutionOutcome::Completed { .. }));
    }

    #[test]
    fn approved_calls_refuse_when_the_target_changed() {
        let tool = Arc::new(Tripwire(
            FakeTool::new("files.delete", RiskLevel::HighRisk, &["files.delete"]),
            Default::default(),
        ));
        let mut registry = ToolRegistry::default();
        registry.register(tool.clone()).unwrap();
        let exec = ToolExecutor::new(registry, PolicyEngine::new(Platform::Linux));
        let approved = Some(ConfirmationSubject::Application {
            application: crate::apps::ApplicationSummary {
                id: "spotify".into(),
                display_name: "Spotify".into(),
                source: crate::apps::AppSource::PackagedApp,
            },
        });
        let out = exec.execute_approved(
            &call("files.delete"),
            &approved,
            &granted(&["files.delete"]),
            &mut ActivityLog::default(),
            1,
        );
        assert!(matches!(out, ExecutionOutcome::SubjectChanged { .. }));
        assert!(!tool.1.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn declined_results_are_audited_with_a_fixed_summary() {
        let mut tool = FakeTool::new("system.get_info", RiskLevel::Safe, &["system.info.read"]);
        tool.result = Err(ToolError::Declined {
            output: ToolOutput::new(Value::Null, "I couldn't find hunter2 on this computer."),
            severity: Severity::Attention,
        });
        let exec = executor_with(vec![tool]);
        let mut log = ActivityLog::default();
        let out = exec.execute(
            &call("system.get_info"),
            &PermissionGrants::default(),
            &mut log,
            1,
            &mut || {},
        );
        assert!(matches!(out, ExecutionOutcome::Declined { .. }));
        assert_eq!(log.recent(1)[0].kind, ActivityKind::ToolDeclined);
        assert!(
            log.recent(10)
                .iter()
                .all(|e| !e.summary.contains("hunter2"))
        );
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
            &mut || {},
        );
        assert!(matches!(out, ExecutionOutcome::Failed { .. }));
        assert!(log.recent(10).iter().all(|e| !e.summary.contains("alice")));
    }
}
