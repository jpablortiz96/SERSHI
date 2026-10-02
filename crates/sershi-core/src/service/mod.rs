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

mod agent;
mod session;

pub use agent::{BrainTicket, Progress};
pub use session::{
    ActionLedger, ActionResult, ConversationSession, LedgerEntry, MAX_LEDGER_ENTRIES, Modality,
    RecallItem, RecallKind, ReferenceSource, SessionTrace,
};
pub use types::{
    BatchStep, BrainTrace, BrainUse, Clock, CommandOutcome, CommandRequest, CommandStatus,
    ConfirmationChoice, ConfirmationDecision, MAX_COMMAND_CHARS, OutcomeDetail, PlanReport,
    PlanStepReport, RejectionReason, ServiceEvent, StepAction, StepStatus, VoiceBusy,
};

use crate::activity::{ActivityEntry, ActivityKind, ActivityLog, NewActivity};
use crate::assistant::{
    AssistantEvent, AssistantSnapshot, AssistantState, StateMachine, TransitionError,
};
use crate::confirmation::{
    ConfirmationError, ConfirmationRequest, ConfirmationStore, ConfirmationSubject, PendingAction,
    PendingConfirmation,
};
use crate::executor::{ExecutionOutcome, ToolExecutor};
use crate::ids::ToolId;
use std::sync::Arc;

use crate::apps::ApplicationSummary;
use crate::apps::risk::{CloseRisk, close_risk};
use crate::brain::AgentBrainPort;
use crate::intent::{Intent, IntentResolver};
use crate::permission::{
    ConfigurablePermission, PermissionGrants, PermissionSetting, PermissionStatus,
};
use crate::policy::{Authorization, DenialReason};
use crate::tool::RiskLevel;
use crate::tool::{Severity, ToolCall, ToolDefinition};
use crate::understanding::{
    ApplicationDirectory, Clarification, Dialogue, InputSource, Interpretation, PendingChange,
    ResolutionTier, SemanticRouterPort, Understanding, Utterance,
};
use std::collections::VecDeque;

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
    // ── Prompt 4: the conversational layer (memory only) ──
    brain: Option<Arc<dyn AgentBrainPort>>,
    /// What SERSHI cannot do yet, for the brain's instructions.
    unavailable: Vec<String>,
    /// The interface language replies are written in.
    response_language: String,
    /// The conversation: recent entities and the action ledger (Gate 4.1),
    /// shared by typed and spoken requests.
    conversation: ConversationSession,
    /// How the current request arrived, what its reference resolved from
    /// and why its action ran (diagnostics).
    modality: Modality,
    reference: Option<ReferenceSource>,
    authorized: Option<Authorization>,
    /// Applications recently asked to close (memory only, bounded), so
    /// Settings can offer per-application settings for them.
    close_candidates: VecDeque<ApplicationSummary>,
    plan: Option<agent::ActivePlan>,
    thinking: Option<agent::Thinking>,
    brain_question: Option<agent::BrainQuestion>,
    ticket_seq: u64,
    plan_seq: u64,
    /// The microphone opened by withdrawing a pending approval: the
    /// transcript that follows is understood conservatively (no context
    /// references, no model), like a request that replaced an approval.
    withdrew_approval: bool,
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
            brain: None,
            unavailable: Vec::new(),
            response_language: "en-US".to_owned(),
            conversation: ConversationSession::default(),
            modality: Modality::Typed,
            reference: None,
            authorized: None,
            close_candidates: VecDeque::new(),
            plan: None,
            thinking: None,
            brain_question: None,
            ticket_seq: 0,
            plan_seq: 0,
            withdrew_approval: false,
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
        if state.is_voice_input() {
            return Err(VoiceBusy);
        }
        let watermark = self.watermark();
        if state.is_busy() {
            // Barge-in (Gate 4.1): the user speaks over work in progress. A
            // brain decision still being prepared is abandoned (its late
            // result is discarded) and a plan stops before its next step;
            // steps that already ran stay done and stay in the ledger.
            self.thinking = None;
            self.cancel_plan();
            self.transition(AssistantEvent::Dismiss, notify);
        }
        let cancelled = self.cancel_confirmations();
        self.withdrew_approval = cancelled.is_some();
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
        let progress = self.begin_transcript(text, asr_confidence, language, notify)?;
        Some(self.drive(progress, notify))
    }

    /// Whether a transcript would already be understood by SERSHI's
    /// deterministic tiers in the current conversation (no model, no side
    /// effects: nothing recorded, no state change). Speech recognition uses
    /// it to skip a language retry for transcripts that already resolve
    /// (Gate 3C.1). Grants nothing: the transcript is still submitted
    /// through [`Self::submit_transcript`].
    pub fn resolves_deterministically(&self, text: &str) -> bool {
        let now = (self.clock)();
        self.understanding.resolves_deterministically(
            &Utterance {
                text: text.trim(),
                source: InputSource::Voice,
                asr_confidence: None,
                language: None,
            },
            &self.dialogue,
            now,
        )
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
        let progress = self.begin_utterance(utterance, notify);
        self.drive(progress, notify)
    }

    /// Starts a typed request without running the brain model or a plan's
    /// steps inline (see [`Progress`]).
    pub fn begin_request(&mut self, request: &CommandRequest, notify: Notify<'_>) -> Progress {
        self.begin_utterance(&Utterance::typed(&request.text), notify)
    }

    /// [`Self::submit_transcript`] without running the brain model or a
    /// plan's steps inline. `None` if the voice interaction was superseded.
    pub fn begin_transcript(
        &mut self,
        text: &str,
        asr_confidence: Option<f32>,
        language: Option<&str>,
        notify: Notify<'_>,
    ) -> Option<Progress> {
        if self.machine.state() != AssistantState::Transcribing {
            return None;
        }
        Some(self.begin_utterance(
            &Utterance {
                text,
                source: InputSource::Voice,
                asr_confidence,
                language,
            },
            notify,
        ))
    }

    /// Requests that are refused before anything happens.
    fn reject_request(&self, text: &str) -> Option<CommandOutcome> {
        if text.is_empty() {
            return Some(CommandOutcome::rejected(
                RejectionReason::Empty,
                "Type or say a command.",
            ));
        }
        if text.chars().count() > MAX_COMMAND_CHARS {
            return Some(CommandOutcome::rejected(
                RejectionReason::TooLong,
                format!("That command is too long. Keep it under {MAX_COMMAND_CHARS} characters."),
            ));
        }
        if self.machine.state().is_busy() {
            return Some(CommandOutcome::rejected(
                RejectionReason::Busy,
                "I'm still working on the previous request.",
            ));
        }
        None
    }

    /// Gate 3C understanding (deterministic tiers, then the semantic
    /// router), with its effect on the open question and the audit.
    fn interpret_gate3c(&mut self, u: &Utterance<'_>, now: u64) -> Interpretation {
        let interpretation = self.understanding.interpret(u, &self.dialogue, now);
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
        interpretation
    }

    /// Carries out an understood intent (one tool call at most).
    fn act_on(
        &mut self,
        intent: Intent,
        replaced: Option<&PendingConfirmation>,
        had_question: bool,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        match intent {
            Intent::UseTool(call) => self.run_call(&call, false, notify),
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
                    replaced.map_or_else(
                        || CommandOutcome::new(CommandStatus::Cancelled, "Nothing was waiting."),
                        cancelled_outcome,
                    )
                }
            }
            Intent::Clarify(clarification) => {
                self.dialogue.ask(clarification.clone(), (self.clock)());
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
                    "I didn't understand that. Try, for example, \"Open Chrome\" or \"How much                      memory am I using?\"",
                )
            }
        }
    }

    /// Completes an outcome: diagnostics, the turn record and the audit.
    fn finish(
        &mut self,
        mut outcome: CommandOutcome,
        text: &str,
        brain: Option<BrainTrace>,
        watermark: u64,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        outcome.brain = brain;
        outcome.session = Some(Box::new(self.session_trace()));
        self.dialogue
            .record((self.clock)(), text, turn_summary(&outcome));
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
            let outcome = self.withdrawn_outcome(&expired, CommandStatus::Expired, notify);
            if expired.request.id == decision.confirmation_id {
                self.flush_activity(watermark, notify);
                return outcome;
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
            Ok(pending) if matches!(pending.action, PendingAction::Batch(_)) => {
                self.decide_batch(pending, decision.decision, notify)
            }
            Ok(pending) if pending.call().is_none() => {
                self.decide_permission(&pending, decision.decision)
            }
            Ok(pending) => match decision.decision {
                ConfirmationChoice::Cancel => {
                    self.record_pending(ActivityKind::ConfirmationCancelled, &pending);
                    self.settle_if_no_pending(notify);
                    self.withdrawn_outcome(&pending, CommandStatus::Cancelled, notify)
                }
                ConfirmationChoice::Approve => {
                    let outcome = self.run_approved(pending, notify);
                    self.with_plan(outcome, true, notify)
                }
            },
        };
        self.flush_activity(watermark, notify);
        outcome
    }

    /// A decision on a plan's step: approved, the plan may continue (the
    /// shell then calls [`Self::advance_plan`] while [`Self::plan_active`]);
    /// declined or expired, the rest of the plan is cancelled.
    fn with_plan(
        &mut self,
        outcome: CommandOutcome,
        approved: bool,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        match self.plan_after_decision(&outcome, approved, notify) {
            Some(Progress::Done(done)) => done,
            Some(Progress::Step(report)) => CommandOutcome {
                plan: Some(report),
                ..outcome
            },
            Some(Progress::Think(_)) | None => outcome,
        }
    }

    /// Expires the pending confirmation if it is overdue, returning the
    /// outcome to show.
    pub fn expire_confirmations(&mut self, notify: Notify<'_>) -> Option<CommandOutcome> {
        let watermark = self.watermark();
        let expired = self.confirmations.expire((self.clock)())?;
        self.record_pending(ActivityKind::ConfirmationExpired, &expired);
        self.settle_if_no_pending(notify);
        let outcome = self.withdrawn_outcome(&expired, CommandStatus::Expired, notify);
        self.flush_activity(watermark, notify);
        Some(outcome)
    }

    /// Returns an attending or waiting assistant to idle (e.g. Escape, the
    /// Command Center was hidden, SERSHI is quitting). A pending approval is
    /// cancelled; its outcome is returned so it can be reported.
    pub fn dismiss(&mut self, notify: Notify<'_>) -> Option<CommandOutcome> {
        let watermark = self.watermark();
        let cancelled = self.cancel_confirmations();
        // Nothing in flight survives a dismissal: a plan stops, a brain
        // decision still running will be discarded.
        self.cancel_plan();
        self.thinking = None;
        self.brain_question = None;
        if self.dialogue.clear_pending() {
            self.record(NewActivity::new(
                ActivityKind::ClarificationCancelled,
                "Cancelled a question",
            ));
        }
        if matches!(
            self.machine.state(),
            AssistantState::Awake
                | AssistantState::Thinking
                | AssistantState::Planning
                | AssistantState::Executing
                | AssistantState::AwaitingConfirmation
                | AssistantState::WaitingForClarification
        ) {
            self.transition(AssistantEvent::Dismiss, notify);
        }
        self.flush_activity(watermark, notify);
        cancelled.as_ref().map(cancelled_outcome)
    }

    /// Runs one tool call through the executor and policy. `in_plan`: a
    /// plan step, whose result does not end the interaction (the plan sets
    /// the final state once).
    fn run_call(&mut self, call: &ToolCall, in_plan: bool, notify: Notify<'_>) -> CommandOutcome {
        if self.machine.state() == AssistantState::Thinking {
            self.transition(AssistantEvent::PlanStarted, notify);
        }
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
        let authorization = authorization_of(&outcome);
        let outcome = self.conclude(outcome, in_plan, notify);
        self.note_result(call, &outcome);
        self.record_action(call, &outcome, authorization);
        outcome
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
        let in_plan = self.plan.is_some();
        let PendingAction::Tool(call) = &pending.action else {
            return CommandOutcome::rejected(
                RejectionReason::UnknownConfirmation,
                "That request is no longer waiting for approval.",
            );
        };
        let execution = self.executor.execute_approved(
            call,
            &pending.request.subject,
            &self.grants,
            &mut self.activity,
            now,
        );
        let authorization = authorization_of(&execution);
        let outcome = self.conclude(execution, in_plan, notify);
        self.note_result(call, &outcome);
        self.record_action(call, &outcome, authorization);
        outcome
    }

    /// Maps an execution outcome to the final state and user-facing outcome.
    fn conclude(
        &mut self,
        outcome: ExecutionOutcome,
        in_plan: bool,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        // A plan step's result does not end the interaction; the plan sets
        // the final state. Confirmation is the exception: it always pauses.
        let end = |service: &mut Self, event: AssistantEvent, notify: Notify<'_>| {
            if !in_plan {
                service.transition(event, notify);
            }
        };
        match outcome {
            ExecutionOutcome::Completed {
                tool_id,
                output,
                duration_ms,
                ..
            } => {
                end(self, AssistantEvent::Completed, notify);
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
                let awaiting = match &draft.subject {
                    Some(ConfirmationSubject::Application { application }) => {
                        Some(OutcomeDetail::AwaitingApproval {
                            applications: vec![application.clone()],
                        })
                    }
                    _ => None,
                };
                // One pending confirmation at a time.
                self.cancel_confirmations();
                match self.confirmations.create(draft, (self.clock)()) {
                    Ok(_) => {
                        self.transition(AssistantEvent::ConfirmationRequested, notify);
                        // The Command Center learns only that approval is
                        // pending; the id goes to the trusted surface alone.
                        CommandOutcome {
                            tool_id: Some(tool_id),
                            detail: awaiting,
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
                        end(self, AssistantEvent::Failed, notify);
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
                end(self, AssistantEvent::AttentionNeeded, notify);
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
                end(self, event, notify);
                CommandOutcome {
                    tool_id: Some(tool_id),
                    data: Some(output.data),
                    ..CommandOutcome::new(status, output.summary)
                }
            }
            ExecutionOutcome::Failed { tool_id, .. } => {
                end(self, AssistantEvent::Failed, notify);
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
                end(self, AssistantEvent::AttentionNeeded, notify);
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
        // What it covered did not run: the ledger says so.
        let ended = CommandOutcome::new(CommandStatus::Cancelled, "");
        match &pending.action {
            PendingAction::Tool(call) => self.record_action(call, &ended, None),
            PendingAction::Batch(items) => {
                for item in items {
                    self.record_action(&item.call, &ended, None);
                }
            }
            PendingAction::Permission { .. } | PendingAction::ApplicationPermission { .. } => {}
        }
        Some(pending)
    }

    fn record_pending(&mut self, kind: ActivityKind, pending: &PendingConfirmation) {
        let subject = pending.request.subject.as_ref().and_then(|s| match s {
            ConfirmationSubject::Application { application } => {
                Some(application.display_name.clone())
            }
            ConfirmationSubject::Applications { applications } => Some(
                applications
                    .iter()
                    .map(|a| a.display_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            ConfirmationSubject::ApplicationPermission { display_name, .. } => {
                Some(display_name.clone())
            }
            ConfirmationSubject::Permission { .. } => None,
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

    // ── Gate 4.1: the conversation session and permissions ──────────────

    /// The conversation's id (changes with "New conversation").
    pub fn conversation_id(&self) -> u64 {
        self.conversation.id
    }

    /// The action ledger (memory only; what SERSHI actually did).
    pub fn ledger(&self) -> &ActionLedger {
        &self.conversation.ledger
    }

    pub(super) fn session_trace(&self) -> SessionTrace {
        let now = (self.clock)();
        SessionTrace {
            session: self.conversation.id,
            modality: self.modality,
            active_entities: self.conversation.entities.apps(now).count(),
            ledger_entries: self.conversation.ledger.len(),
            last_action: self.conversation.ledger.last_success().map(|e| RecallItem {
                action: e.action,
                application: e.application.clone(),
                result: e.result,
            }),
            reference: self.reference,
            authorization: self.authorized,
        }
    }

    /// Records a real tool execution in the action ledger. Nothing that did
    /// not run (pending approval, denied, cancelled) is recorded.
    fn record_action(
        &mut self,
        call: &ToolCall,
        outcome: &CommandOutcome,
        authorization: Option<Authorization>,
    ) {
        let Some(result) = ActionResult::from_status(outcome.status) else {
            return;
        };
        let application = self.call_app(call).or_else(|| {
            let id = outcome
                .data
                .as_ref()?
                .get("application")?
                .get("id")?
                .as_str()?;
            self.understanding
                .catalog_names()
                .into_iter()
                .map(|n| n.application)
                .find(|a| a.id == id)
        });
        if authorization.is_some() {
            self.authorized = authorization;
        }
        let plan = self.plan_active();
        self.conversation.ledger.record(
            (self.clock)(),
            self.conversation.request,
            call.tool_id.clone(),
            agent::step_action(&call.tool_id),
            application,
            result,
            call.origin,
            plan,
            authorization,
        );
    }

    /// The configurable permissions and their current settings.
    pub fn permission_settings(&self) -> Vec<PermissionStatus> {
        ConfigurablePermission::ALL
            .into_iter()
            .map(|permission| PermissionStatus {
                permission,
                setting: self.grants.setting(permission),
                default_setting: permission.default_setting(),
            })
            .collect()
    }

    /// Whether a high-risk tool uses this permission. Such a permission is
    /// never configurable (and none is today).
    fn guards_high_risk(&self, permission: ConfigurablePermission) -> bool {
        let id = permission.permission_id();
        self.executor
            .registry()
            .definitions()
            .any(|d| d.risk == RiskLevel::HighRisk && d.permissions.contains(&id))
    }

    /// Applies settings the user stored earlier (start-up). A setting for
    /// a permission a high-risk tool uses is ignored.
    pub fn load_permission_settings(
        &mut self,
        settings: &[(ConfigurablePermission, PermissionSetting)],
    ) {
        for &(permission, setting) in settings {
            if !self.guards_high_risk(permission) {
                self.grants.configure(permission, setting);
            }
        }
    }

    /// A permission change requested in Settings. Making a permission more
    /// restrictive applies at once. Making it less restrictive ("Always
    /// allow") is itself an approval: it waits in the trusted confirmation
    /// window like a sensitive action, and only [`Self::decide`] applies it.
    /// Neither voice, a model nor the Command Center can grant it.
    pub fn request_permission_change(
        &mut self,
        permission: ConfigurablePermission,
        setting: PermissionSetting,
        notify: Notify<'_>,
    ) -> Result<PermissionChange, PermissionChangeError> {
        if self.guards_high_risk(permission) {
            return Err(PermissionChangeError::NotConfigurable);
        }
        let current = self.grants.setting(permission);
        if current == setting {
            return Ok(PermissionChange::Unchanged);
        }
        let watermark = self.watermark();
        if !setting.loosens(current) {
            self.apply_permission(permission, setting);
            self.flush_activity(watermark, notify);
            return Ok(PermissionChange::Applied);
        }
        let state = self.machine.state();
        if state.is_busy()
            || state.is_voice_input()
            || state == AssistantState::AwaitingConfirmation
            || self.plan.is_some()
            || self.thinking.is_some()
        {
            return Err(PermissionChangeError::Busy);
        }
        // One pending confirmation at a time.
        self.cancel_confirmations();
        self.confirmations
            .create_permission(permission, setting, (self.clock)())
            .map_err(|_| PermissionChangeError::Unavailable)?;
        self.record(NewActivity::new(
            ActivityKind::ConfirmationRequired,
            "A permission change is waiting for your approval",
        ));
        self.flush_activity(watermark, notify);
        Ok(PermissionChange::NeedsConfirmation)
    }

    fn apply_permission(&mut self, permission: ConfigurablePermission, setting: PermissionSetting) {
        let before = self.grants.setting(permission);
        self.grants.configure(permission, setting);
        self.record(NewActivity::new(
            ActivityKind::PermissionChanged,
            format!(
                "{}: {} → {}",
                permission_label(permission),
                setting_label(before),
                setting_label(setting)
            ),
        ));
    }

    fn decide_permission(
        &mut self,
        pending: &PendingConfirmation,
        choice: ConfirmationChoice,
    ) -> CommandOutcome {
        let approve = choice == ConfirmationChoice::Approve;
        self.record_pending(
            if approve {
                ActivityKind::ConfirmationApproved
            } else {
                ActivityKind::ConfirmationCancelled
            },
            pending,
        );
        match &pending.action {
            PendingAction::Permission {
                permission,
                setting,
            } if approve => self.apply_permission(*permission, *setting),
            PendingAction::ApplicationPermission {
                app_id,
                display_name,
                setting,
            } if approve => self.apply_app_permission(app_id, display_name, *setting),
            PendingAction::Permission { .. } | PendingAction::ApplicationPermission { .. } => {}
            PendingAction::Tool(_) | PendingAction::Batch(_) => {
                return CommandOutcome::rejected(
                    RejectionReason::UnknownConfirmation,
                    "That request is no longer waiting for approval.",
                );
            }
        }
        permission_outcome(pending, choice == ConfirmationChoice::Approve).unwrap_or_else(|| {
            CommandOutcome::new(CommandStatus::Cancelled, "Nothing was changed.")
        })
    }

    /// The outcome of a confirmation that ended without approval
    /// (cancelled, expired, withdrawn): what it covered did not run, and
    /// the ledger says so.
    fn withdrawn_outcome(
        &mut self,
        pending: &PendingConfirmation,
        status: CommandStatus,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        if let Some(outcome) = permission_outcome(pending, false) {
            return outcome;
        }
        let ended = CommandOutcome::new(status, "");
        match &pending.action {
            PendingAction::Tool(call) => {
                self.record_action(call, &ended, None);
                let outcome = if status == CommandStatus::Expired {
                    expired_outcome(Some(&pending.request.tool_id))
                } else {
                    cancelled_outcome(pending)
                };
                self.with_plan(outcome, false, notify)
            }
            PendingAction::Batch(items) => {
                let steps = items
                    .iter()
                    .filter_map(|item| {
                        self.record_action(&item.call, &ended, None);
                        batch_app(&item.subject).map(|application| BatchStep {
                            application,
                            status: StepStatus::Cancelled,
                        })
                    })
                    .collect();
                CommandOutcome {
                    detail: Some(OutcomeDetail::CloseBatch { steps }),
                    ..CommandOutcome::new(status, "Cancelled. Nothing was changed.")
                }
            }
            PendingAction::Permission { .. } | PendingAction::ApplicationPermission { .. } => {
                cancelled_outcome(pending)
            }
        }
    }

    /// The trusted confirmation window could not be shown: the pending
    /// approval is withdrawn (an approval nobody can see must not wait),
    /// whatever it covered does not run, and SERSHI says so.
    pub fn withdraw_confirmation(&mut self, notify: Notify<'_>) -> Option<CommandOutcome> {
        let watermark = self.watermark();
        let pending = self.confirmations.cancel_all()?;
        self.record_pending(ActivityKind::ConfirmationCancelled, &pending);
        self.settle_if_no_pending(notify);
        let withdrawn = self.withdrawn_outcome(&pending, CommandStatus::Cancelled, notify);
        self.cancel_plan();
        self.flush_activity(watermark, notify);
        Some(CommandOutcome {
            detail: Some(OutcomeDetail::ConfirmationUnavailable),
            plan: withdrawn.plan,
            ..CommandOutcome::new(
                CommandStatus::Failed,
                "The confirmation window couldn't be shown, so nothing was done.",
            )
        })
    }

    /// A decision on a grouped close: approved, exactly the stored calls run
    /// (each target re-verified); cancelled, none of them do.
    fn decide_batch(
        &mut self,
        pending: PendingConfirmation,
        choice: ConfirmationChoice,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        if choice == ConfirmationChoice::Cancel {
            self.record_pending(ActivityKind::ConfirmationCancelled, &pending);
            self.settle_if_no_pending(notify);
            return self.withdrawn_outcome(&pending, CommandStatus::Cancelled, notify);
        }
        let PendingAction::Batch(items) = &pending.action else {
            return CommandOutcome::rejected(
                RejectionReason::UnknownConfirmation,
                "That request is no longer waiting for approval.",
            );
        };
        if self.machine.state() != AssistantState::AwaitingConfirmation {
            return CommandOutcome::rejected(
                RejectionReason::UnknownConfirmation,
                "That request is no longer waiting for approval.",
            );
        }
        self.record_pending(ActivityKind::ConfirmationApproved, &pending);
        self.transition(AssistantEvent::ConfirmationApproved, notify);
        let mut steps = Vec::with_capacity(items.len());
        for item in items {
            let now = (self.clock)();
            let execution = self.executor.execute_approved(
                &item.call,
                &item.subject,
                &self.grants,
                &mut self.activity,
                now,
            );
            let authorization = authorization_of(&execution);
            let outcome = self.conclude(execution, true, notify);
            self.note_result(&item.call, &outcome);
            self.record_action(&item.call, &outcome, authorization);
            if let Some(application) = batch_app(&item.subject) {
                steps.push(BatchStep {
                    application,
                    status: step_status(outcome.status),
                });
            }
        }
        self.finish_batch(steps, notify)
    }

    /// Ends a grouped close whose every step has run or been decided.
    pub(super) fn finish_batch(
        &mut self,
        steps: Vec<BatchStep>,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        let done = steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count();
        let (status, event) = if done == steps.len() {
            (CommandStatus::Completed, AssistantEvent::Completed)
        } else if done > 0 {
            (CommandStatus::Partial, AssistantEvent::AttentionNeeded)
        } else {
            (CommandStatus::Failed, AssistantEvent::Failed)
        };
        if self.machine.state().is_busy() {
            self.transition(event, notify);
        }
        CommandOutcome {
            detail: Some(OutcomeDetail::CloseBatch { steps }),
            ..CommandOutcome::new(status, format!("Closed {done} application(s)."))
        }
    }

    /// Per-application close settings, plus applications recently asked to
    /// close (so Settings can offer them). Trusted names only.
    pub fn application_permissions(&self) -> Vec<AppPermissionStatus> {
        let mut out: Vec<AppPermissionStatus> = self
            .grants
            .app_permissions()
            .into_iter()
            .map(|p| AppPermissionStatus {
                close_risk: CloseRisk::Unknown,
                app_id: p.app_id,
                display_name: p.display_name,
                setting: Some(p.close),
            })
            .collect();
        for app in &self.close_candidates {
            if let Some(existing) = out.iter_mut().find(|o| o.app_id == app.id) {
                existing.close_risk = close_risk(app);
            } else {
                out.push(AppPermissionStatus {
                    app_id: app.id.clone(),
                    display_name: app.display_name.clone(),
                    setting: None,
                    close_risk: close_risk(app),
                });
            }
        }
        out
    }

    /// Applies per-application settings the user stored earlier.
    pub fn load_application_permissions(
        &mut self,
        settings: &[(String, String, PermissionSetting)],
    ) {
        for (id, name, setting) in settings {
            self.grants.set_app_close(id, name, *setting);
        }
    }

    /// Settings asked to change one application's close setting. "Ask every
    /// time" applies at once; "Always allow" waits for the trusted window.
    /// Only an application SERSHI knows (a stored setting, or one recently
    /// asked to close) can be configured.
    pub fn request_app_permission_change(
        &mut self,
        app_id: &str,
        setting: PermissionSetting,
        notify: Notify<'_>,
    ) -> Result<PermissionChange, PermissionChangeError> {
        let Some(known) = self
            .application_permissions()
            .into_iter()
            .find(|a| a.app_id == app_id)
        else {
            return Err(PermissionChangeError::NotConfigurable);
        };
        if known.setting == Some(setting) {
            return Ok(PermissionChange::Unchanged);
        }
        let watermark = self.watermark();
        if setting == PermissionSetting::AskEveryTime {
            self.apply_app_permission(app_id, &known.display_name, setting);
            self.flush_activity(watermark, notify);
            return Ok(PermissionChange::Applied);
        }
        let state = self.machine.state();
        if state.is_busy()
            || state.is_voice_input()
            || state == AssistantState::AwaitingConfirmation
            || self.plan.is_some()
            || self.thinking.is_some()
        {
            return Err(PermissionChangeError::Busy);
        }
        self.cancel_confirmations();
        self.confirmations
            .create_app_permission(app_id, &known.display_name, setting, (self.clock)())
            .map_err(|_| PermissionChangeError::Unavailable)?;
        self.record(NewActivity::new(
            ActivityKind::ConfirmationRequired,
            "A permission change is waiting for your approval",
        ));
        self.flush_activity(watermark, notify);
        Ok(PermissionChange::NeedsConfirmation)
    }

    fn apply_app_permission(
        &mut self,
        app_id: &str,
        display_name: &str,
        setting: PermissionSetting,
    ) {
        let before = self.grants.app_close(app_id);
        if self.grants.set_app_close(app_id, display_name, setting) {
            self.record(
                NewActivity::new(
                    ActivityKind::PermissionChanged,
                    format!(
                        "Close {display_name}: {} → {}",
                        before.map_or("Default", setting_label),
                        setting_label(setting)
                    ),
                )
                .subject_opt(Some(display_name.to_owned())),
            );
        }
    }

    /// Remembers an application asked to close (memory only, bounded).
    pub(super) fn remember_close_candidate(&mut self, app: &ApplicationSummary) {
        self.close_candidates.retain(|a| a.id != app.id);
        self.close_candidates.push_front(app.clone());
        self.close_candidates.truncate(MAX_CLOSE_CANDIDATES);
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
    if let Some(outcome) = permission_outcome(pending, false) {
        return outcome;
    }
    CommandOutcome {
        tool_id: Some(pending.request.tool_id.clone()),
        ..CommandOutcome::new(CommandStatus::Cancelled, "Cancelled. Nothing was changed.")
    }
}

/// What a permission request in Settings led to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum PermissionChange {
    /// More restrictive: applied immediately.
    Applied,
    /// Already set that way.
    Unchanged,
    /// Less restrictive: waiting in the trusted confirmation window.
    NeedsConfirmation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionChangeError {
    /// SERSHI is working or another approval is pending; try again.
    Busy,
    /// A high-risk tool uses it: never configurable.
    NotConfigurable,
    /// No secure confirmation could be created.
    Unavailable,
}

fn permission_label(permission: ConfigurablePermission) -> &'static str {
    match permission {
        ConfigurablePermission::OpenApplications => "Open applications",
        ConfigurablePermission::CloseApplications => "Close applications",
        ConfigurablePermission::SystemInformation => "System information",
    }
}

fn setting_label(setting: PermissionSetting) -> &'static str {
    match setting {
        PermissionSetting::AlwaysAllow => "Always allow",
        PermissionSetting::AskEveryTime => "Ask every time",
    }
}

/// The outcome of a permission confirmation (`None` for a tool one).
fn permission_outcome(pending: &PendingConfirmation, applied: bool) -> Option<CommandOutcome> {
    let detail = match &pending.action {
        PendingAction::Permission {
            permission,
            setting,
        } => OutcomeDetail::Permission {
            permission: *permission,
            setting: *setting,
            applied,
        },
        PendingAction::ApplicationPermission {
            app_id,
            display_name,
            setting,
        } => OutcomeDetail::ApplicationPermission {
            app_id: app_id.clone(),
            display_name: display_name.clone(),
            setting: *setting,
            applied,
        },
        PendingAction::Tool(_) | PendingAction::Batch(_) => return None,
    };
    let (status, reply) = if applied {
        (CommandStatus::Completed, "Permission updated.")
    } else {
        (CommandStatus::Cancelled, "Nothing was changed.")
    };
    Some(CommandOutcome {
        detail: Some(detail),
        ..CommandOutcome::new(status, reply)
    })
}

/// The application a grouped close's item targets.
fn batch_app(subject: &Option<ConfirmationSubject>) -> Option<ApplicationSummary> {
    match subject {
        Some(ConfirmationSubject::Application { application }) => Some(application.clone()),
        _ => None,
    }
}

pub(super) fn step_status(status: CommandStatus) -> StepStatus {
    match status {
        CommandStatus::Completed => StepStatus::Completed,
        CommandStatus::NeedsConfirmation => StepStatus::NeedsConfirmation,
        CommandStatus::Unresolved => StepStatus::Unresolved,
        CommandStatus::Cancelled | CommandStatus::Expired => StepStatus::Cancelled,
        _ => StepStatus::Failed,
    }
}

/// Most applications remembered as recently asked to close.
const MAX_CLOSE_CANDIDATES: usize = 8;

/// One application's close setting as Settings shows it (Gate 4.1.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AppPermissionStatus {
    pub app_id: String,
    pub display_name: String,
    /// The stored setting; `None` follows the defaults.
    pub setting: Option<PermissionSetting>,
    pub close_risk: CloseRisk,
}

fn authorization_of(outcome: &ExecutionOutcome) -> Option<Authorization> {
    match outcome {
        ExecutionOutcome::Completed { authorization, .. } => Some(*authorization),
        _ => None,
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
