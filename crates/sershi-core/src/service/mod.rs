//! The assistant service: the application layer that turns a user command
//! into a state-machine-driven, policy-checked, audited outcome.
//!
//! ```text
//! CommandRequest → validate → Thinking (intent) → Planning (policy, resolve)
//!     ├─ allowed           → Executing → Success / Warning / Error
//!     └─ needs approval    → AwaitingConfirmation
//!            decide(approve) → Executing → …   decide(cancel) / expiry → Idle
//! ```
//!
//! Invariant: the service never approves on its own. Approval enters only
//! through [`AssistantService::decide`], which the desktop shell accepts only
//! from the trusted confirmation surface; intent resolution, tools and model
//! output have no path to it.
//!
//! The service is synchronous and owns no threads or timers; the desktop shell
//! wraps it in a mutex and schedules settle and expiry timers.

mod types;

#[cfg(test)]
mod tests;

pub use types::{
    Clock, CommandOutcome, CommandRequest, CommandStatus, ConfirmationChoice, ConfirmationDecision,
    MAX_COMMAND_CHARS, OutcomeDetail, RejectionReason, ServiceEvent,
};

use crate::activity::{ActivityEntry, ActivityKind, ActivityLog, NewActivity};
use crate::assistant::{
    AssistantEvent, AssistantSnapshot, AssistantState, StateMachine, TransitionError,
};
use crate::confirmation::{
    ConfirmationError, ConfirmationRequest, ConfirmationStore, ConfirmationSubject,
    PendingConfirmation,
};
use crate::executor::{ExecutionOutcome, ToolExecutor};
use crate::ids::ToolId;
use crate::intent::{Intent, IntentResolver};
use crate::permission::PermissionGrants;
use crate::policy::DenialReason;
use crate::tool::{Severity, ToolCall, ToolDefinition};

#[derive(Debug)]
pub struct AssistantService {
    machine: StateMachine,
    executor: ToolExecutor,
    resolver: Box<dyn IntentResolver>,
    grants: PermissionGrants,
    activity: ActivityLog,
    confirmations: ConfirmationStore,
    clock: Clock,
}

type Notify<'a> = &'a mut dyn FnMut(ServiceEvent);

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
            confirmations: ConfirmationStore::default(),
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

    /// The confirmation currently awaiting a decision, if any. For the
    /// desktop shell's trusted surface only; never sent to the Command Center.
    pub fn pending_confirmation(&self) -> Option<ConfirmationRequest> {
        self.confirmations.current().cloned()
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

    pub fn submit(&mut self, request: &CommandRequest, notify: Notify<'_>) -> CommandOutcome {
        let text = request.text.trim();
        if text.is_empty() {
            return CommandOutcome::rejected(RejectionReason::Empty, "Type or say a command.");
        }
        if text.chars().count() > MAX_COMMAND_CHARS {
            return CommandOutcome::rejected(
                RejectionReason::TooLong,
                format!("That command is too long. Keep it under {MAX_COMMAND_CHARS} characters."),
            );
        }
        if self.machine.state().is_busy() {
            return CommandOutcome::rejected(
                RejectionReason::Busy,
                "I'm still working on the previous request.",
            );
        }

        let watermark = self.watermark();
        // A new request replaces any pending approval.
        self.cancel_confirmations();
        // The command text itself is deliberately not recorded.
        self.record(NewActivity::new(
            ActivityKind::CommandReceived,
            "Command received",
        ));
        self.transition(AssistantEvent::RequestReceived, notify);

        let outcome = match self.resolver.resolve(text) {
            Intent::UseTool(call) => self.run_tool(&call, notify),
            Intent::Answer(topic) => {
                self.transition(AssistantEvent::Completed, notify);
                CommandOutcome {
                    detail: Some(OutcomeDetail::Answer { topic }),
                    ..CommandOutcome::new(CommandStatus::Answered, topic.canonical_text())
                }
            }
            Intent::NotYetAvailable(capability) => {
                let (label, milestone) = (capability.label, capability.milestone);
                self.record(NewActivity::new(
                    ActivityKind::CapabilityUnavailable,
                    format!("{label} is planned for {milestone}"),
                ));
                self.transition(AssistantEvent::AttentionNeeded, notify);
                CommandOutcome {
                    detail: Some(OutcomeDetail::Unavailable {
                        capability: capability.id.to_owned(),
                        milestone: milestone.to_owned(),
                    }),
                    ..CommandOutcome::new(
                        CommandStatus::Unavailable,
                        format!("{label} isn't available yet — it's planned for {milestone}."),
                    )
                }
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
        self.flush_activity(watermark, notify);
        outcome
    }

    /// Applies the user's decision on the pending confirmation. Only the
    /// stored call can run, only once and only before expiry; the decision
    /// carries nothing but the id and the choice.
    pub fn decide(
        &mut self,
        decision: &ConfirmationDecision,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        let now = (self.clock)();
        let watermark = self.watermark();
        // An overdue confirmation expires before any decision is considered.
        if let Some(expired) = self.confirmations.expire(now) {
            self.record_pending(ActivityKind::ConfirmationExpired, &expired);
            self.settle_if_no_pending(notify);
            if expired.request.id == decision.confirmation_id {
                self.flush_activity(watermark, notify);
                return expired_outcome(Some(&expired.request.tool_id));
            }
        }
        let outcome = match self.confirmations.take(&decision.confirmation_id, now) {
            Err(ConfirmationError::Expired) => {
                self.settle_if_no_pending(notify);
                expired_outcome(None)
            }
            Err(_) => CommandOutcome::rejected(
                RejectionReason::UnknownConfirmation,
                "That request is no longer waiting for approval.",
            ),
            Ok(pending) => match decision.decision {
                ConfirmationChoice::Cancel => {
                    self.record_pending(ActivityKind::ConfirmationCancelled, &pending);
                    self.settle_if_no_pending(notify);
                    cancelled_outcome(&pending)
                }
                ConfirmationChoice::Approve => self.run_approved(pending, notify),
            },
        };
        self.flush_activity(watermark, notify);
        outcome
    }

    /// Expires the pending confirmation if it is overdue, returning the
    /// outcome to show.
    pub fn expire_confirmations(&mut self, notify: Notify<'_>) -> Option<CommandOutcome> {
        let watermark = self.watermark();
        let expired = self.confirmations.expire((self.clock)())?;
        self.record_pending(ActivityKind::ConfirmationExpired, &expired);
        self.settle_if_no_pending(notify);
        self.flush_activity(watermark, notify);
        Some(expired_outcome(Some(&expired.request.tool_id)))
    }

    /// Returns an attending or waiting assistant to idle (e.g. Escape, the
    /// Command Center was hidden, SERSHI is quitting). A pending approval is
    /// cancelled; its outcome is returned so it can be reported.
    pub fn dismiss(&mut self, notify: Notify<'_>) -> Option<CommandOutcome> {
        let watermark = self.watermark();
        let cancelled = self.cancel_confirmations();
        if matches!(
            self.machine.state(),
            AssistantState::Awake | AssistantState::AwaitingConfirmation
        ) {
            self.transition(AssistantEvent::Dismiss, notify);
        }
        self.flush_activity(watermark, notify);
        cancelled.as_ref().map(cancelled_outcome)
    }

    fn run_tool(&mut self, call: &ToolCall, notify: Notify<'_>) -> CommandOutcome {
        self.transition(AssistantEvent::PlanStarted, notify);
        let now = (self.clock)();
        let outcome = {
            let machine = &mut self.machine;
            let mut on_execute = || {
                if let Ok(snapshot) = machine.apply(AssistantEvent::ExecutionStarted) {
                    notify(ServiceEvent::State(snapshot));
                }
            };
            self.executor
                .execute(call, &self.grants, &mut self.activity, now, &mut on_execute)
        };
        self.conclude(outcome, notify)
    }

    fn run_approved(&mut self, pending: PendingConfirmation, notify: Notify<'_>) -> CommandOutcome {
        if self.machine.state() != AssistantState::AwaitingConfirmation {
            // Defensive: approvals are only meaningful while waiting.
            return CommandOutcome::rejected(
                RejectionReason::UnknownConfirmation,
                "That request is no longer waiting for approval.",
            );
        }
        self.record_pending(ActivityKind::ConfirmationApproved, &pending);
        self.transition(AssistantEvent::ConfirmationApproved, notify);
        let now = (self.clock)();
        let outcome = self.executor.execute_approved(
            &pending.call,
            &pending.request.subject,
            &self.grants,
            &mut self.activity,
            now,
        );
        self.conclude(outcome, notify)
    }

    /// Maps an execution outcome to the final state and user-facing outcome.
    fn conclude(&mut self, outcome: ExecutionOutcome, notify: Notify<'_>) -> CommandOutcome {
        match outcome {
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
            ExecutionOutcome::ConfirmationRequired(draft) => {
                let tool_id = draft.call.tool_id.clone();
                let tool_name = self.tool_name(&tool_id);
                // One pending confirmation at a time.
                self.cancel_confirmations();
                match self.confirmations.create(draft, (self.clock)()) {
                    Ok(_) => {
                        self.transition(AssistantEvent::ConfirmationRequested, notify);
                        // The Command Center learns only that approval is
                        // pending; the id goes to the trusted surface alone.
                        CommandOutcome {
                            tool_id: Some(tool_id),
                            ..CommandOutcome::new(
                                CommandStatus::NeedsConfirmation,
                                format!(
                                    "{tool_name} is waiting for your approval in the confirmation window."
                                ),
                            )
                        }
                    }
                    Err(_) => {
                        // Fail closed: no secure id, no confirmation, no action.
                        self.record(
                            NewActivity::new(ActivityKind::ToolFailed, "Tool failed")
                                .tool(&tool_id),
                        );
                        self.transition(AssistantEvent::Failed, notify);
                        CommandOutcome {
                            tool_id: Some(tool_id),
                            ..CommandOutcome::new(
                                CommandStatus::Failed,
                                format!(
                                    "I couldn't complete {tool_name}. The system didn't return \
                                     the information — try again in a moment."
                                ),
                            )
                        }
                    }
                }
            }
            ExecutionOutcome::Denied { tool_id, reason } => {
                self.transition(AssistantEvent::AttentionNeeded, notify);
                let tool_name = self.tool_name(&tool_id);
                let why = match &reason {
                    DenialReason::UnknownTool => "that tool isn't installed",
                    DenialReason::Prohibited => "it is blocked by policy",
                    DenialReason::UnsupportedPlatform => "it isn't supported on this computer",
                    DenialReason::PermissionDenied { .. } => {
                        "you've denied the permission it needs"
                    }
                };
                CommandOutcome {
                    tool_id: Some(tool_id),
                    detail: Some(OutcomeDetail::Denied { reason }),
                    ..CommandOutcome::new(
                        CommandStatus::Denied,
                        format!("I can't use {tool_name} because {why}."),
                    )
                }
            }
            ExecutionOutcome::Declined {
                tool_id,
                output,
                severity,
            } => {
                let (event, status) = match severity {
                    Severity::Attention => {
                        (AssistantEvent::AttentionNeeded, CommandStatus::Unresolved)
                    }
                    Severity::Failure => (AssistantEvent::Failed, CommandStatus::Failed),
                };
                self.transition(event, notify);
                CommandOutcome {
                    tool_id: Some(tool_id),
                    data: Some(output.data),
                    ..CommandOutcome::new(status, output.summary)
                }
            }
            ExecutionOutcome::Failed { tool_id, .. } => {
                self.transition(AssistantEvent::Failed, notify);
                let tool_name = self.tool_name(&tool_id);
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
            ExecutionOutcome::SubjectChanged { tool_id } => {
                self.transition(AssistantEvent::AttentionNeeded, notify);
                expired_outcome(Some(&tool_id))
            }
        }
    }

    fn tool_name(&self, tool_id: &ToolId) -> String {
        self.executor
            .registry()
            .get(tool_id)
            .map(|t| t.definition().name.clone())
            .unwrap_or_else(|| tool_id.to_string())
    }

    fn cancel_confirmations(&mut self) -> Option<PendingConfirmation> {
        let pending = self.confirmations.cancel_all()?;
        self.record_pending(ActivityKind::ConfirmationCancelled, &pending);
        Some(pending)
    }

    fn record_pending(&mut self, kind: ActivityKind, pending: &PendingConfirmation) {
        let subject = pending.request.subject.as_ref().map(|s| match s {
            ConfirmationSubject::Application { application } => application.display_name.clone(),
        });
        self.record_confirmation(kind, subject, Some(&pending.request.tool_id));
    }

    fn record_confirmation(
        &mut self,
        kind: ActivityKind,
        subject: Option<String>,
        tool: Option<&ToolId>,
    ) {
        let summary = match kind {
            ActivityKind::ConfirmationApproved => "Approved a pending action",
            ActivityKind::ConfirmationExpired => "A pending approval expired",
            _ => "Cancelled a pending action",
        };
        let mut activity = NewActivity::new(kind, summary).subject_opt(subject);
        if let Some(tool) = tool {
            activity = activity.tool(tool);
        }
        self.record(activity);
    }

    fn settle_if_no_pending(&mut self, notify: Notify<'_>) {
        if self.confirmations.is_empty()
            && self.machine.state() == AssistantState::AwaitingConfirmation
        {
            self.transition(AssistantEvent::Dismiss, notify);
        }
    }

    fn watermark(&self) -> u64 {
        self.activity.recent(1).first().map_or(0, |e| e.id)
    }

    fn flush_activity(&self, watermark: u64, notify: Notify<'_>) {
        for entry in self.activity.recent(usize::MAX).into_iter().rev() {
            if entry.id > watermark {
                notify(ServiceEvent::Activity(entry));
            }
        }
    }

    fn transition(&mut self, event: AssistantEvent, notify: Notify<'_>) {
        // Every transition used here is valid from the state the service
        // leaves the machine in; a rejected one indicates a logic error and
        // is surfaced by tests rather than panicking in production.
        let result = self.machine.apply(event);
        debug_assert!(result.is_ok(), "invalid transition: {result:?}");
        if let Ok(snapshot) = result {
            notify(ServiceEvent::State(snapshot));
        }
    }
}

fn cancelled_outcome(pending: &PendingConfirmation) -> CommandOutcome {
    CommandOutcome {
        tool_id: Some(pending.request.tool_id.clone()),
        ..CommandOutcome::new(CommandStatus::Cancelled, "Cancelled. Nothing was changed.")
    }
}

fn expired_outcome(tool_id: Option<&ToolId>) -> CommandOutcome {
    CommandOutcome {
        tool_id: tool_id.cloned(),
        ..CommandOutcome::new(
            CommandStatus::Expired,
            "This request expired. Ask SERSHI again if you still want to do it.",
        )
    }
}
