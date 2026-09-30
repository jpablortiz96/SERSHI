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
//! from the trusted confirmation surface; intent resolution, tools, model
//! output and voice have no path to it.
//!
//! Voice (push-to-talk) adds states around the same pipeline:
//!
//! ```text
//! begin_listening → Listening ─ end_listening → Transcribing
//!     ─ submit_transcript(text) → submit(text)   (exactly the typed path)
//!     ─ voice_unusable → Warning   (no speech / unclear: nothing runs)
//! outcome ─ begin_speaking → Speaking ─ end_speaking → Idle
//! ```
//!
//! Understanding (Gate 3C, [`crate::understanding`]) may instead ask which
//! application was meant:
//!
//! ```text
//! Thinking ─ Clarify → WaitingForClarification
//!     ─ next request (the answer, typed or spoken) → Thinking → …
//!     ─ dismiss / expiry → Idle
//! ```
//!
//! The question and a few recent turns live in the service's [`Dialogue`]
//! (memory only). An answer only selects among the trusted candidates that
//! were offered; the resulting call goes through the same policy and
//! confirmation as any other.
//!
//! The service is synchronous and owns no threads or timers; the desktop shell
//! wraps it in a mutex and schedules settle and expiry timers.

mod types;

#[cfg(test)]
mod tests;

pub use types::{
    Clock, CommandOutcome, CommandRequest, CommandStatus, ConfirmationChoice, ConfirmationDecision,
    MAX_COMMAND_CHARS, OutcomeDetail, RejectionReason, ServiceEvent, VoiceBusy,
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
use std::sync::Arc;

use crate::intent::{Intent, IntentResolver};
use crate::permission::PermissionGrants;
use crate::policy::DenialReason;
use crate::tool::{Severity, ToolCall, ToolDefinition};
use crate::understanding::{
    ApplicationDirectory, Clarification, Dialogue, InputSource, PendingChange, ResolutionTier,
    SemanticRouterPort, Understanding, Utterance,
};

#[derive(Debug)]
pub struct AssistantService {
    machine: StateMachine,
    executor: ToolExecutor,
    understanding: Understanding,
    /// The open question and recent turns (memory only).
    dialogue: Dialogue,
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
            understanding: Understanding::new(resolver),
            dialogue: Dialogue::default(),
            grants,
            activity: ActivityLog::default(),
            confirmations: ConfirmationStore::default(),
            clock,
        }
    }

    /// Lets understanding resolve names against the trusted catalog
    /// (similar names, clarifications). Without it, application names are
    /// passed to the tools as said, as before Gate 3C.
    pub fn with_applications(mut self, apps: Arc<dyn ApplicationDirectory>) -> Self {
        self.understanding.set_applications(apps);
        self
    }

    /// Installs (or removes) the local semantic model.
    pub fn set_semantic_router(&mut self, router: Option<Arc<dyn SemanticRouterPort>>) {
        self.understanding.set_router(router);
    }

    /// A request may come soon (the microphone opened).
    pub fn prepare_understanding(&self) {
        self.understanding.prepare();
    }

    /// The question SERSHI is waiting on, if it has not expired.
    pub fn pending_clarification(&self) -> Option<Clarification> {
        self.dialogue
            .pending((self.clock)())
            .map(|p| p.clarification.clone())
    }

    /// Expires an overdue question and leaves the waiting state. Returns
    /// whether one expired.
    pub fn expire_clarification(&mut self, notify: Notify<'_>) -> bool {
        let watermark = self.watermark();
        if self.dialogue.expire((self.clock)()).is_none() {
            return false;
        }
        self.record(NewActivity::new(
            ActivityKind::ClarificationExpired,
            "A question expired unanswered",
        ));
        if self.machine.state() == AssistantState::WaitingForClarification {
            self.transition(AssistantEvent::Dismiss, notify);
        }
        self.flush_activity(watermark, notify);
        true
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

    // ── Voice ─────────────────────────────────────────────────────────────

    /// Push-to-talk: the microphone is about to open. A pending approval is
    /// cancelled first — a spoken request, like any new request, replaces
    /// it — so the microphone and an approval never overlap and nothing said
    /// can reach it. Returns the cancellation to report, if any.
    pub fn begin_listening(
        &mut self,
        notify: Notify<'_>,
    ) -> Result<Option<CommandOutcome>, VoiceBusy> {
        let state = self.machine.state();
        if state.is_busy() || state.is_voice_input() {
            return Err(VoiceBusy);
        }
        let watermark = self.watermark();
        let cancelled = self.cancel_confirmations();
        if state == AssistantState::AwaitingConfirmation {
            self.transition(AssistantEvent::Dismiss, notify);
        }
        self.record(NewActivity::new(
            ActivityKind::MicrophoneOn,
            "Microphone on",
        ));
        self.transition(AssistantEvent::StartListening, notify);
        self.flush_activity(watermark, notify);
        Ok(cancelled.as_ref().map(cancelled_outcome))
    }

    /// The microphone closed after `duration_ms`; recognition starts.
    /// Returns false if SERSHI was no longer listening.
    pub fn end_listening(&mut self, duration_ms: u32, notify: Notify<'_>) -> bool {
        if self.machine.state() != AssistantState::Listening {
            return false;
        }
        let watermark = self.watermark();
        self.record_microphone_off(duration_ms);
        self.transition(AssistantEvent::CaptureEnded, notify);
        self.flush_activity(watermark, notify);
        true
    }

    /// Submits a recognised transcript through exactly the same path as
    /// typed text ([`Self::submit`]). `None` if the voice interaction was
    /// superseded (cancelled, or a typed command arrived first).
    ///
    /// `asr_confidence` (mean token probability) and `language` only inform
    /// understanding; a low confidence makes SERSHI ask rather than act.
    pub fn submit_transcript(
        &mut self,
        text: &str,
        asr_confidence: Option<f32>,
        language: Option<&str>,
        notify: Notify<'_>,
    ) -> Option<CommandOutcome> {
        if self.machine.state() != AssistantState::Transcribing {
            return None;
        }
        Some(self.submit_utterance(
            &Utterance {
                text,
                source: InputSource::Voice,
                asr_confidence,
                language,
            },
            notify,
        ))
    }

    /// Recognition heard nothing usable (silence, unclear speech). Nothing
    /// runs; the assistant shows it needs attention.
    pub fn voice_unusable(&mut self, notify: Notify<'_>) -> bool {
        if self.machine.state() != AssistantState::Transcribing {
            return false;
        }
        self.transition(AssistantEvent::AttentionNeeded, notify);
        true
    }

    /// The voice interaction was cancelled (Escape, the Command Center was
    /// hidden, a typed command took over). `mic_ms` is how long the
    /// microphone was open if it was still capturing.
    pub fn cancel_voice(&mut self, mic_ms: Option<u32>, notify: Notify<'_>) -> bool {
        self.end_voice(mic_ms, AssistantEvent::Dismiss, notify)
    }

    /// Capture or recognition failed.
    pub fn voice_failed(&mut self, mic_ms: Option<u32>, notify: Notify<'_>) -> bool {
        self.end_voice(mic_ms, AssistantEvent::Failed, notify)
    }

    fn end_voice(
        &mut self,
        mic_ms: Option<u32>,
        event: AssistantEvent,
        notify: Notify<'_>,
    ) -> bool {
        let state = self.machine.state();
        if !state.is_voice_input() {
            return false;
        }
        let watermark = self.watermark();
        if state == AssistantState::Listening {
            self.record_microphone_off(mic_ms.unwrap_or(0));
        }
        self.transition(event, notify);
        self.flush_activity(watermark, notify);
        true
    }

    fn record_microphone_off(&mut self, duration_ms: u32) {
        self.record(
            NewActivity::new(ActivityKind::MicrophoneOff, "Microphone off").duration(duration_ms),
        );
    }

    /// A spoken reply started playing. Returns whether the state now shows
    /// Speaking; it never replaces work in progress, voice input or a
    /// pending approval (the reply still plays; the more important state
    /// stays visible).
    pub fn begin_speaking(&mut self, notify: Notify<'_>) -> bool {
        let state = self.machine.state();
        let may_show =
            state.is_transient() || matches!(state, AssistantState::Idle | AssistantState::Awake);
        if !may_show {
            return false;
        }
        self.transition(AssistantEvent::SpeechStarted, notify);
        true
    }

    /// The spoken reply finished or was stopped.
    pub fn end_speaking(&mut self, notify: Notify<'_>) -> bool {
        if self.machine.state() != AssistantState::Speaking {
            return false;
        }
        self.transition(AssistantEvent::SpeechEnded, notify);
        true
    }

    pub fn submit(&mut self, request: &CommandRequest, notify: Notify<'_>) -> CommandOutcome {
        self.submit_utterance(&Utterance::typed(&request.text), notify)
    }

    fn submit_utterance(
        &mut self,
        utterance: &Utterance<'_>,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        let text = utterance.text.trim();
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
        let replaced = self.cancel_confirmations();
        // The command text itself is deliberately not recorded.
        self.record(NewActivity::new(
            ActivityKind::CommandReceived,
            "Command received",
        ));
        self.transition(AssistantEvent::RequestReceived, notify);

        let now = (self.clock)();
        if self.dialogue.expire(now).is_some() {
            self.record(NewActivity::new(
                ActivityKind::ClarificationExpired,
                "A question expired unanswered",
            ));
        }
        let had_question = self.dialogue.pending(now).is_some();
        let interpretation =
            self.understanding
                .interpret(&Utterance { text, ..*utterance }, &self.dialogue, now);
        match &interpretation.pending {
            PendingChange::Keep => {}
            PendingChange::Clear => {
                self.dialogue.clear_pending();
            }
            PendingChange::Ask(question) => self.dialogue.ask(question.clone(), now),
        }
        if matches!(interpretation.intent, Intent::UseTool(_))
            && let Some(summary) = interpreted_summary(interpretation.trace.tier)
        {
            self.record(NewActivity::new(ActivityKind::CommandInterpreted, summary));
        }

        let mut outcome = match interpretation.intent {
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
            Intent::Cancel => {
                self.transition(AssistantEvent::Dismiss, notify);
                if replaced.is_none() && had_question {
                    self.record(NewActivity::new(
                        ActivityKind::ClarificationCancelled,
                        "Cancelled a question",
                    ));
                    CommandOutcome::new(CommandStatus::Cancelled, "Cancelled. Nothing was changed.")
                } else {
                    replaced.as_ref().map_or_else(
                        || CommandOutcome::new(CommandStatus::Cancelled, "Nothing was waiting."),
                        cancelled_outcome,
                    )
                }
            }
            Intent::Clarify(clarification) => {
                self.record(NewActivity::new(
                    ActivityKind::ClarificationRequested,
                    "Asked which application was meant",
                ));
                self.transition(AssistantEvent::ClarificationRequested, notify);
                let reply = clarification.canonical_text();
                CommandOutcome {
                    detail: Some(OutcomeDetail::Clarification { clarification }),
                    ..CommandOutcome::new(CommandStatus::NeedsClarification, reply)
                }
            }
            Intent::NotUnderstood => {
                self.transition(AssistantEvent::AttentionNeeded, notify);
                CommandOutcome::new(
                    CommandStatus::NotUnderstood,
                    "I didn't understand that. Try, for example, \"Open Chrome\" or \"How much \
                     memory am I using?\"",
                )
            }
        };
        outcome.understood = interpretation.understood;
        outcome.understanding = Some(interpretation.trace);
        self.dialogue.record(now, text, turn_summary(&outcome));
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
        if self.dialogue.clear_pending() {
            self.record(NewActivity::new(
                ActivityKind::ClarificationCancelled,
                "Cancelled a question",
            ));
        }
        if matches!(
            self.machine.state(),
            AssistantState::Awake
                | AssistantState::AwaitingConfirmation
                | AssistantState::WaitingForClarification
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

/// Audit text for a request understood through a repair (never the words).
fn interpreted_summary(tier: ResolutionTier) -> Option<&'static str> {
    Some(match tier {
        ResolutionTier::Fuzzy => "Understood a similar application name",
        ResolutionTier::Phonetic => "Understood a similar-sounding application name",
        ResolutionTier::VerbRepair => "Understood a misheard command word",
        ResolutionTier::Context => "Understood the answer to a question",
        ResolutionTier::Semantic => "Understood with the local language model",
        _ => return None,
    })
}

/// A short, structured memory of what a turn did, for context.
fn turn_summary(outcome: &CommandOutcome) -> Option<String> {
    let data = outcome.data.as_ref()?;
    let app = data.get("application")?.get("displayName")?.as_str()?;
    let verb = match data.get("kind")?.as_str()? {
        "opened" => "opened",
        "closeRequested" => "closed",
        _ => return None,
    };
    Some(format!("{verb} {app}"))
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
