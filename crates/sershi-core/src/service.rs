//! The assistant service: the application layer that turns a user command
//! into a state-machine-driven, policy-checked, audited outcome.
//!
//! ```text
//! CommandRequest → validate → RequestReceived → intent → tool call
//!     → executor (policy → tool) → outcome state → CommandOutcome
//! ```
//!
//! The service is synchronous and owns no threads or timers; the desktop shell
//! wraps it in a mutex and schedules the settle-back-to-idle transition.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::activity::{ActivityEntry, ActivityKind, ActivityLog, NewActivity};
use crate::assistant::{
    AssistantEvent, AssistantSnapshot, AssistantState, StateMachine, TransitionError,
};
use crate::executor::{ConfirmationRequest, ExecutionOutcome, ToolExecutor};
use crate::ids::ToolId;
use crate::intent::{Intent, IntentResolver};
use crate::permission::PermissionGrants;
use crate::policy::DenialReason;
use crate::tool::{ToolCall, ToolDefinition};

/// Longest command accepted over IPC.
pub const MAX_COMMAND_CHARS: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandRequest {
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum CommandStatus {
    /// A tool ran successfully.
    Completed,
    /// SERSHI answered without acting.
    Answered,
    NeedsConfirmation,
    Denied,
    /// Understood, but the capability is not built yet.
    Unavailable,
    NotUnderstood,
    Failed,
    /// The request itself was invalid (empty, too long, or SERSHI was busy).
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    pub status: CommandStatus,
    /// What SERSHI says back. Composed from tool summaries and fixed copy.
    pub reply: String,
    pub tool_id: Option<ToolId>,
    pub data: Option<Value>,
    pub confirmation: Option<ConfirmationRequest>,
    pub duration_ms: Option<u32>,
}

impl CommandOutcome {
    fn new(status: CommandStatus, reply: impl Into<String>) -> Self {
        Self {
            status,
            reply: reply.into(),
            tool_id: None,
            data: None,
            confirmation: None,
            duration_ms: None,
        }
    }
}

/// Changes the host should broadcast to every surface.
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceEvent {
    State(AssistantSnapshot),
    Activity(ActivityEntry),
}

pub type Clock = fn() -> u64;

#[derive(Debug)]
pub struct AssistantService {
    machine: StateMachine,
    executor: ToolExecutor,
    resolver: Box<dyn IntentResolver>,
    grants: PermissionGrants,
    activity: ActivityLog,
    clock: Clock,
}

impl AssistantService {
    pub fn new(
        executor: ToolExecutor,
        resolver: Box<dyn IntentResolver>,
        grants: PermissionGrants,
        clock: Clock,
    ) -> Self {
        Self {
            machine: StateMachine::default(),
            executor,
            resolver,
            grants,
            activity: ActivityLog::default(),
            clock,
        }
    }

    pub fn snapshot(&self) -> AssistantSnapshot {
        self.machine.snapshot()
    }

    pub fn recent_activity(&self, limit: usize) -> Vec<ActivityEntry> {
        self.activity.recent(limit)
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.executor.registry().definitions().cloned().collect()
    }

    pub fn record(&mut self, activity: NewActivity) -> ActivityEntry {
        self.activity.record((self.clock)(), activity)
    }

    pub fn apply(&mut self, event: AssistantEvent) -> Result<AssistantSnapshot, TransitionError> {
        self.machine.apply(event)
    }

    pub fn apply_if_current(
        &mut self,
        revision: u64,
        event: AssistantEvent,
    ) -> Option<AssistantSnapshot> {
        self.machine.apply_if_current(revision, event)
    }

    pub fn set_preview(&mut self, preview: Option<AssistantState>) -> AssistantSnapshot {
        self.machine.set_preview(preview)
    }

    pub fn submit(
        &mut self,
        request: &CommandRequest,
        notify: &mut dyn FnMut(ServiceEvent),
    ) -> CommandOutcome {
        let text = request.text.trim();
        if text.is_empty() {
            return CommandOutcome::new(CommandStatus::Rejected, "Type or say a command.");
        }
        if text.chars().count() > MAX_COMMAND_CHARS {
            return CommandOutcome::new(
                CommandStatus::Rejected,
                format!("That command is too long. Keep it under {MAX_COMMAND_CHARS} characters."),
            );
        }
        if self.machine.state().is_busy() {
            return CommandOutcome::new(
                CommandStatus::Rejected,
                "I'm still working on the previous request.",
            );
        }

        let watermark = self.activity.recent(1).first().map_or(0, |e| e.id);
        // The command text itself is deliberately not recorded.
        self.record(NewActivity::new(
            ActivityKind::CommandReceived,
            "Command received",
        ));
        self.transition(AssistantEvent::RequestReceived, notify);

        let outcome = match self.resolver.resolve(text) {
            Intent::UseTool(call) => self.run_tool(&call, notify),
            Intent::Reply(reply) => {
                self.transition(AssistantEvent::Completed, notify);
                CommandOutcome::new(CommandStatus::Answered, reply)
            }
            Intent::NotYetAvailable {
                capability,
                milestone,
            } => {
                self.record(NewActivity::new(
                    ActivityKind::CapabilityUnavailable,
                    format!("{capability} is planned for {milestone}"),
                ));
                self.transition(AssistantEvent::AttentionNeeded, notify);
                CommandOutcome::new(
                    CommandStatus::Unavailable,
                    format!("{capability} isn't available yet — it's planned for {milestone}."),
                )
            }
            Intent::NotUnderstood => {
                self.transition(AssistantEvent::AttentionNeeded, notify);
                CommandOutcome::new(
                    CommandStatus::NotUnderstood,
                    "I didn't understand that. Until an AI provider is connected I can answer \
                     a few system questions — try \"How much memory am I using?\"",
                )
            }
        };

        for entry in self.activity.recent(usize::MAX).into_iter().rev() {
            if entry.id > watermark {
                notify(ServiceEvent::Activity(entry));
            }
        }
        outcome
    }

    fn run_tool(
        &mut self,
        call: &ToolCall,
        notify: &mut dyn FnMut(ServiceEvent),
    ) -> CommandOutcome {
        self.transition(AssistantEvent::ExecutionStarted, notify);
        let now = (self.clock)();
        let tool_name = self
            .executor
            .registry()
            .get(&call.tool_id)
            .map(|t| t.definition().name.clone())
            .unwrap_or_else(|| call.tool_id.to_string());

        match self
            .executor
            .execute(call, &self.grants, &mut self.activity, now)
        {
            ExecutionOutcome::Completed {
                tool_id,
                output,
                duration_ms,
            } => {
                self.transition(AssistantEvent::Completed, notify);
                CommandOutcome {
                    tool_id: Some(tool_id),
                    data: Some(output.data),
                    duration_ms: Some(duration_ms),
                    ..CommandOutcome::new(CommandStatus::Completed, output.summary)
                }
            }
            ExecutionOutcome::ConfirmationRequired { request } => {
                self.transition(AssistantEvent::AttentionNeeded, notify);
                CommandOutcome {
                    tool_id: Some(request.tool_id.clone()),
                    confirmation: Some(request),
                    ..CommandOutcome::new(
                        CommandStatus::NeedsConfirmation,
                        format!("{tool_name} needs your approval before it can run."),
                    )
                }
            }
            ExecutionOutcome::Denied { tool_id, reason } => {
                self.transition(AssistantEvent::AttentionNeeded, notify);
                let why = match reason {
                    DenialReason::UnknownTool => "that tool isn't installed",
                    DenialReason::Prohibited => "it is blocked by policy",
                    DenialReason::UnsupportedPlatform => "it isn't supported on this computer",
                    DenialReason::PermissionDenied { .. } => {
                        "you've denied the permission it needs"
                    }
                };
                CommandOutcome {
                    tool_id: Some(tool_id),
                    ..CommandOutcome::new(
                        CommandStatus::Denied,
                        format!("I can't use {tool_name} because {why}."),
                    )
                }
            }
            ExecutionOutcome::Failed { tool_id, .. } => {
                self.transition(AssistantEvent::Failed, notify);
                CommandOutcome {
                    tool_id: Some(tool_id),
                    ..CommandOutcome::new(
                        CommandStatus::Failed,
                        format!(
                            "I couldn't complete {tool_name}. The system didn't return the \
                             information — try again in a moment."
                        ),
                    )
                }
            }
        }
    }

    fn transition(&mut self, event: AssistantEvent, notify: &mut dyn FnMut(ServiceEvent)) {
        // Every transition used by `submit` is valid from the state `submit`
        // leaves the machine in; a rejected one indicates a logic error and is
        // surfaced by tests rather than panicking in production.
        let result = self.machine.apply(event);
        debug_assert!(result.is_ok(), "invalid transition in submit: {result:?}");
        if let Ok(snapshot) = result {
            notify(ServiceEvent::State(snapshot));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::builtin::{register_all, test_support::FakeSystem};
    use crate::intent::KeywordIntentResolver;
    use crate::platform::Platform;
    use crate::policy::PolicyEngine;
    use crate::ports::PortError;
    use crate::tool::ToolRegistry;

    fn service_with(system: FakeSystem, grants: PermissionGrants) -> AssistantService {
        let mut registry = ToolRegistry::default();
        register_all(&mut registry, Arc::new(system)).unwrap();
        AssistantService::new(
            ToolExecutor::new(registry, PolicyEngine::new(Platform::current())),
            Box::new(KeywordIntentResolver),
            grants,
            || 1_000,
        )
    }

    fn submit(service: &mut AssistantService, text: &str) -> (CommandOutcome, Vec<ServiceEvent>) {
        let mut events = Vec::new();
        let outcome = service.submit(
            &CommandRequest {
                text: text.to_owned(),
            },
            &mut |e| events.push(e),
        );
        (outcome, events)
    }

    fn states(events: &[ServiceEvent]) -> Vec<AssistantState> {
        events
            .iter()
            .filter_map(|e| match e {
                ServiceEvent::State(s) => Some(s.state),
                ServiceEvent::Activity(_) => None,
            })
            .collect()
    }

    #[test]
    fn memory_question_runs_the_full_pipeline() {
        let mut service = service_with(FakeSystem::ok(), PermissionGrants::default());
        let (outcome, events) = submit(&mut service, "How much RAM am I using?");

        assert_eq!(outcome.status, CommandStatus::Completed);
        assert!(outcome.reply.contains("12.0 GB of 32.0 GB"));
        assert_eq!(
            states(&events),
            [
                AssistantState::Thinking,
                AssistantState::Executing,
                AssistantState::Success
            ]
        );
        let kinds: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                ServiceEvent::Activity(a) => Some(a.kind),
                ServiceEvent::State(_) => None,
            })
            .collect();
        assert_eq!(
            kinds,
            [
                ActivityKind::CommandReceived,
                ActivityKind::ToolRequested,
                ActivityKind::ToolCompleted
            ]
        );
    }

    #[test]
    fn user_command_text_never_reaches_the_activity_log() {
        let mut service = service_with(FakeSystem::ok(), PermissionGrants::default());
        submit(&mut service, "memory, and my password is hunter2");
        assert!(
            service
                .recent_activity(50)
                .iter()
                .all(|e| !e.summary.contains("hunter2"))
        );
    }

    #[test]
    fn revoked_permission_blocks_builtin_tools() {
        let mut service = service_with(FakeSystem::ok(), PermissionGrants::empty());
        let (outcome, events) = submit(&mut service, "memory");
        assert_eq!(outcome.status, CommandStatus::NeedsConfirmation);
        assert!(
            outcome.data.is_none(),
            "no data may be returned before approval"
        );
        assert_eq!(states(&events).last(), Some(&AssistantState::Warning));
    }

    #[test]
    fn platform_failures_become_error_state_with_useful_copy() {
        let mut service = service_with(
            FakeSystem(Err(PortError::Platform("0x000041AF".into()))),
            PermissionGrants::default(),
        );
        let (outcome, events) = submit(&mut service, "memory");
        assert_eq!(outcome.status, CommandStatus::Failed);
        assert!(
            !outcome.reply.contains("0x"),
            "no raw error codes in user copy"
        );
        assert_eq!(states(&events).last(), Some(&AssistantState::Error));
    }

    #[test]
    fn unavailable_and_unknown_requests_are_honest() {
        let mut service = service_with(FakeSystem::ok(), PermissionGrants::default());
        let (open, _) = submit(&mut service, "Open Spotify");
        assert_eq!(open.status, CommandStatus::Unavailable);
        let (unknown, _) = submit(&mut service, "zxqv");
        assert_eq!(unknown.status, CommandStatus::NotUnderstood);
    }

    #[test]
    fn invalid_requests_are_rejected_without_state_changes() {
        let mut service = service_with(FakeSystem::ok(), PermissionGrants::default());
        let before = service.snapshot();
        for text in ["   ".to_owned(), "a".repeat(MAX_COMMAND_CHARS + 1)] {
            let (outcome, events) = submit(&mut service, &text);
            assert_eq!(outcome.status, CommandStatus::Rejected);
            assert!(events.is_empty());
        }
        assert_eq!(service.snapshot(), before);
    }

    #[test]
    fn a_new_command_may_interrupt_a_transient_outcome() {
        let mut service = service_with(FakeSystem::ok(), PermissionGrants::default());
        submit(&mut service, "memory");
        assert_eq!(service.snapshot().state, AssistantState::Success);
        let (outcome, _) = submit(&mut service, "cpu");
        assert_eq!(outcome.status, CommandStatus::Completed);
    }
}
