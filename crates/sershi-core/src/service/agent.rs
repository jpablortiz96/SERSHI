//! The conversational layer and plan orchestration (Prompt 4).
//!
//! ```text
//! request ─► begin_utterance
//!   ├─ conversational deterministic tier (no model):
//!   │    references ("ciérralo"), compound commands ("abre Chrome y Outlook"),
//!   │    elliptical follow-ups ("ahora PowerPoint")
//!   ├─ Gate 3C (fast path, semantic router)
//!   └─ Agent Brain model ─► Progress::Think(ticket)
//!         the shell runs the model *outside* the service lock, with a
//!         timeout, then complete_brain(ticket) — accepted only if the
//!         ticket is still current (cancelled or replaced requests are
//!         discarded and can never act)
//! plans ─► Progress::Step ─► advance_plan, one step per call
//!   each step: its own ToolCall → executor → policy → confirmation
//! ```
//!
//! A plan never receives blanket authority: a sensitive step pauses the
//! plan in the trusted confirmation window; approving it runs that step
//! only; declining or letting it expire cancels the rest.

use std::sync::Arc;

use serde_json::Value;

use super::session::{ActionResult, Modality, RecallItem, RecallKind, ReferenceSource};
use super::types::{
    BrainTrace, BrainUse, CommandOutcome, CommandStatus, OutcomeDetail, PlanReport, PlanStepReport,
    ServiceEvent, StepAction, StepStatus,
};
use super::{AssistantService, Notify};
use crate::activity::{ActivityKind, NewActivity};
use crate::apps::ApplicationSummary;
use crate::apps::tools::{CLOSE_APPLICATION, OPEN_APPLICATION};
use crate::assistant::{AssistantEvent, AssistantState};
use crate::brain::context::{AppEvent, Referent, SystemFact};
use crate::brain::route::{self, UnderstandingRoute};
use crate::brain::{
    AgentBrainPort, BRAIN_PROMPT_VERSION, BrainDecision, BrainError, BrainRequest, MAX_PLAN_STEPS,
    manifest,
};
use crate::confirmation::ConfirmationSubject;
use crate::ids::ToolId;
use crate::intent::Intent;
use crate::tool::{CallOrigin, ToolCall};
use crate::understanding::grammar::AppAction;
use crate::understanding::{
    Clarification, ClarificationKind, InputSource, UnderstoodAs, Utterance,
};

/// How long a question the brain asked stays open.
const BRAIN_QUESTION_TTL_MS: u64 = 60_000;
/// Most applications offered to the brain in one request.
const MAX_OFFERED: usize = crate::brain::contract::MAX_APPS;

/// What to do next with a request.
#[derive(Debug)]
pub enum Progress {
    /// Finished: show this outcome.
    Done(CommandOutcome),
    /// The brain model must decide. Run `ticket.brain.decide(&ticket.request)`
    /// without holding the service, then call
    /// [`AssistantService::complete_brain`].
    Think(BrainTicket),
    /// A plan has more steps: call [`AssistantService::advance_plan`] with
    /// this id (the shell releases the service between steps, so the user
    /// can cancel).
    Step(PlanReport),
}

/// One pending brain decision.
#[derive(Debug, Clone)]
pub struct BrainTicket {
    pub id: u64,
    pub request: BrainRequest,
    pub brain: Arc<dyn AgentBrainPort>,
}

/// The request waiting on the brain (memory only).
#[derive(Debug, Clone)]
pub(super) struct Thinking {
    id: u64,
    text: String,
    asr_confidence: Option<f32>,
    language: Option<String>,
    source: InputSource,
    /// Applications offered to the model, in handle order.
    offered: Vec<ApplicationSummary>,
    watermark: u64,
    had_question: bool,
}

/// A question the brain asked, still open.
#[derive(Debug, Clone)]
pub(super) struct BrainQuestion {
    message: String,
    options: Vec<ApplicationSummary>,
    expires_at: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ActivePlan {
    id: u64,
    calls: Vec<ToolCall>,
    steps: Vec<PlanStepReport>,
    /// Index of an earlier step each one depends on.
    depends: Vec<Option<usize>>,
    cursor: usize,
    /// The step waiting in the confirmation window.
    awaiting: Option<usize>,
    trace: BrainTrace,
}

impl ActivePlan {
    fn report(&self) -> PlanReport {
        PlanReport {
            id: self.id,
            steps: self.steps.clone(),
            done: self.cursor >= self.calls.len() && self.awaiting.is_none(),
        }
    }
}

/// What the conversational deterministic tier resolved.
enum Resolved {
    Intent(Intent, Option<UnderstoodAs>),
    Plan(Vec<ToolCall>),
    TooLong,
}

pub(super) fn step_action(tool_id: &ToolId) -> StepAction {
    match tool_id.as_str() {
        OPEN_APPLICATION => StepAction::Open,
        CLOSE_APPLICATION => StepAction::Close,
        "system.get_memory" => StepAction::Memory,
        "system.get_cpu" => StepAction::Cpu,
        _ => StepAction::SystemInfo,
    }
}

fn app_call(action: AppAction, app: &ApplicationSummary, origin: CallOrigin) -> Option<ToolCall> {
    let tool = match action {
        AppAction::Open => OPEN_APPLICATION,
        AppAction::Close => CLOSE_APPLICATION,
    };
    Some(ToolCall::new(
        ToolId::new(tool).ok()?,
        serde_json::json!({ "application": app.id }),
        origin,
    ))
}

/// A part made only of fillers ("Oye", "OK SERSHI", "bueno").
fn is_filler(part: &str) -> bool {
    let tokens = crate::understanding::grammar::tokens(part);
    crate::understanding::grammar::skip_fillers(&tokens) >= tokens.len()
}

/// Ordinal words for "el primero", "the second one", "o terceiro".
fn ordinal(text: &str) -> Option<usize> {
    let keys: Vec<String> = text
        .split_whitespace()
        .map(crate::apps::normalize::normalize)
        .collect();
    for (words, n) in [
        (
            &["primero", "primera", "first", "primeiro", "primeira"][..],
            0,
        ),
        (&["segundo", "segunda", "second"][..], 1),
        (
            &["tercero", "tercera", "third", "terceiro", "terceira"][..],
            2,
        ),
    ] {
        if keys.iter().any(|k| words.contains(&k.as_str())) {
            return Some(n);
        }
    }
    None
}

impl AssistantService {
    /// Installs (or removes) the Agent Brain. `unavailable` lists what
    /// SERSHI cannot do yet (from the platform's capability report), so the
    /// brain never promises it.
    pub fn set_agent_brain(
        &mut self,
        brain: Option<Arc<dyn AgentBrainPort>>,
        unavailable: Vec<String>,
    ) {
        self.brain = brain;
        self.unavailable = unavailable;
    }

    /// The interface language replies are written in (`es-419`, …).
    pub fn set_response_language(&mut self, tag: &str) {
        self.response_language = tag.chars().take(16).collect();
    }

    /// A plan is running or waiting for a step's approval.
    pub fn plan_active(&self) -> Option<u64> {
        self.plan.as_ref().map(|p| p.id)
    }

    /// The brain model may be needed soon (the microphone opened).
    pub fn prepare_brain(&self) {
        if let Some(brain) = &self.brain {
            brain.prepare();
        }
    }

    /// "New conversation": forgets the conversation — entities, the action
    /// ledger, turns, open questions, the current plan — under a new
    /// session id, so nothing said before can be referred to. Keeps
    /// preferences, permissions and models; the security audit is separate
    /// and unaffected. Cancels a pending approval and any running plan.
    pub fn reset_conversation(&mut self, notify: Notify<'_>) -> Option<CommandOutcome> {
        let cancelled = self.dismiss(notify);
        self.conversation.reset();
        self.dialogue.reset();
        self.brain_question = None;
        self.reference = None;
        self.authorized = None;
        cancelled
    }

    // ── Entry ─────────────────────────────────────────────────────────────

    /// Starts a request: everything up to the first point where the shell
    /// must wait (the brain model) or step (a plan). Typed and spoken
    /// requests both come here.
    pub fn begin_utterance(&mut self, utterance: &Utterance<'_>, notify: Notify<'_>) -> Progress {
        let text = utterance.text.trim();
        if let Some(rejected) = self.reject_request(text) {
            return Progress::Done(rejected);
        }
        let watermark = self.watermark();
        // One conversation for typed and spoken requests (Gate 4.1).
        self.conversation.request += 1;
        self.modality = match utterance.source {
            InputSource::Typed => Modality::Typed,
            InputSource::Voice => Modality::Voice,
        };
        self.reference = None;
        self.authorized = None;
        // A new request replaces any pending approval, plan or brain
        // decision: none of them can continue afterwards.
        let replaced = self.cancel_confirmations();
        self.cancel_plan();
        self.thinking = None;
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
        if self
            .brain_question
            .as_ref()
            .is_some_and(|q| now >= q.expires_at)
        {
            self.brain_question = None;
        }
        let had_question = self.dialogue.pending(now).is_some();
        let u = Utterance { text, ..*utterance };
        let deterministic = self
            .understanding
            .resolves_deterministically(&u, &self.dialogue, now);
        // Words said while an approval was waiting are most likely meant as
        // that approval ("yes", "do it", "close it"). They withdrew it; they
        // must never re-create it through context or the model. Such a
        // request is understood by Gate 3C alone.
        let withdrew = std::mem::take(&mut self.withdrew_approval);
        let conversational = replaced.is_none() && !withdrew;

        // 0. A negated action never acts, and a command line is never run,
        //    whatever a model would read into the text.
        // A hypothetical is talk about an action: the brain explains it;
        // without the brain, nothing is done.
        if route::hypothetical(text) && !had_question && conversational && self.brain.is_some() {
            return self.think(&u, watermark, had_question, now);
        }
        let refusal = if route::command_request(text) {
            Some(crate::intent::AnswerTopic::NoCommands)
        } else if (route::negated_action(text) || route::hypothetical(text)) && !had_question {
            Some(crate::intent::AnswerTopic::NoAction)
        } else {
            None
        };
        if let Some(topic) = refusal {
            let outcome = self.act_on(
                Intent::Answer(topic),
                replaced.as_ref(),
                had_question,
                notify,
            );
            return Progress::Done(self.finish(outcome, text, None, watermark, notify));
        }

        // Recall ("what did you just open?"): answered from the action
        // ledger — what really ran — never from a model's recollection.
        // Acts on nothing, so it is safe in every mode.
        if !had_question && let Some(kind) = route::recall(text) {
            return Progress::Done(self.recall(kind, text, watermark, notify));
        }

        // Plural selections: "los dos" answering "which one?", or "close
        // both" after opening two. Only among trusted candidates already
        // offered or acted on; each action is its own plan step, with its
        // own policy and confirmation.
        if conversational && !tampering_or_brain_question(text, self.brain_question.is_some()) {
            let selected = if had_question {
                self.plural_answer(text, now)
            } else {
                self.plural_reference(text, now)
            };
            if let Some((calls, source)) = selected {
                self.reference = Some(source);
                if had_question {
                    self.dialogue.clear_pending();
                    self.record(NewActivity::new(
                        ActivityKind::CommandInterpreted,
                        "Understood the answer to a question",
                    ));
                }
                let trace = BrainTrace {
                    route: UnderstandingRoute::AgentBrain,
                    brain: Some(BrainUse::Deterministic),
                    model_ms: None,
                    prompt_version: BRAIN_PROMPT_VERSION,
                    steps: calls.len(),
                };
                return self.start_plan(calls, trace, text, watermark, notify);
            }
        }

        // 1. Compound commands: every part resolved by the trusted tiers
        //    becomes a plan — before the whole-sentence fast path, which
        //    would otherwise silently drop the later parts.
        let tampering = route::tampering(text);
        if conversational && !tampering && !had_question && self.brain_question.is_none() {
            match self.compound(text, now) {
                Some(Resolved::Plan(calls)) => {
                    let trace = BrainTrace {
                        route: UnderstandingRoute::AgentBrain,
                        brain: Some(BrainUse::Deterministic),
                        model_ms: None,
                        prompt_version: BRAIN_PROMPT_VERSION,
                        steps: calls.len(),
                    };
                    return self.start_plan(calls, trace, text, watermark, notify);
                }
                Some(Resolved::TooLong) => {
                    return Progress::Done(self.plan_too_long(text, watermark, notify));
                }
                Some(Resolved::Intent(intent, understood)) => {
                    let trace = BrainTrace {
                        route: UnderstandingRoute::AgentBrain,
                        brain: Some(BrainUse::Deterministic),
                        model_ms: None,
                        prompt_version: BRAIN_PROMPT_VERSION,
                        steps: 0,
                    };
                    let mut outcome = self.act_on(intent, replaced.as_ref(), had_question, notify);
                    outcome.understood = understood;
                    return Progress::Done(self.finish(
                        outcome,
                        text,
                        Some(trace),
                        watermark,
                        notify,
                    ));
                }
                None => {}
            }
        }

        // 2. References and follow-ups (no model).
        if conversational && !had_question && !deterministic && self.brain_question.is_none() {
            let resolved = self.conversational(text, now);
            if resolved.is_some() {
                self.reference = Some(ReferenceSource::Entities);
            }
            match resolved {
                Some(Resolved::Intent(intent, understood)) => {
                    let trace = BrainTrace {
                        route: UnderstandingRoute::AgentBrain,
                        brain: Some(BrainUse::Deterministic),
                        model_ms: None,
                        prompt_version: BRAIN_PROMPT_VERSION,
                        steps: 0,
                    };
                    let mut outcome = self.act_on(intent, replaced.as_ref(), had_question, notify);
                    outcome.understood = understood;
                    return Progress::Done(self.finish(
                        outcome,
                        text,
                        Some(trace),
                        watermark,
                        notify,
                    ));
                }
                Some(Resolved::Plan(calls)) => {
                    let trace = BrainTrace {
                        route: UnderstandingRoute::AgentBrain,
                        brain: Some(BrainUse::Deterministic),
                        model_ms: None,
                        prompt_version: BRAIN_PROMPT_VERSION,
                        steps: calls.len(),
                    };
                    return self.start_plan(calls, trace, text, watermark, notify);
                }
                Some(Resolved::TooLong) => {
                    return Progress::Done(self.plan_too_long(text, watermark, notify));
                }
                None => {}
            }
        }

        // 3. Questions and open brain conversations go to the brain model —
        //    including long questions a system keyword merely mentions
        //    ("explain what 80 % RAM means" is not "read my memory").
        let long_question = route::is_question(text) && text.split_whitespace().count() >= 7;
        if conversational
            && !had_question
            && self.brain.is_some()
            && (self.brain_question.is_some()
                || (route::is_question(text) && !deterministic)
                || (long_question && self.keyword_only(&u, now)))
        {
            return self.think(&u, watermark, had_question, now);
        }

        // 4. Gate 3C (fast path, semantic router); the brain model takes
        //    what Gate 3C does not understand.
        let interpretation = self.interpret_gate3c(&u, now);
        if conversational
            && matches!(interpretation.intent, Intent::NotUnderstood)
            && !had_question
            && self.brain.is_some()
        {
            return self.think(&u, watermark, had_question, now);
        }
        let route = if interpretation.trace.semantic == crate::understanding::SemanticUse::Used {
            UnderstandingRoute::SemanticRouter
        } else {
            UnderstandingRoute::FastPath
        };
        let brain = BrainTrace {
            route,
            brain: if self.brain.is_some() {
                None
            } else {
                Some(BrainUse::NotInstalled)
            },
            model_ms: None,
            prompt_version: BRAIN_PROMPT_VERSION,
            steps: 0,
        };
        let understood = interpretation.understood.clone();
        let trace = interpretation.trace.clone();
        let mut outcome = self.act_on(
            interpretation.intent,
            replaced.as_ref(),
            had_question,
            notify,
        );
        outcome.understood = understood;
        outcome.understanding = Some(trace);
        Progress::Done(self.finish(outcome, text, Some(brain), watermark, notify))
    }

    /// Drives a request to its end inline (tests, and callers that may
    /// block). The desktop shell uses [`Self::begin_utterance`],
    /// [`Self::complete_brain`] and [`Self::advance_plan`] instead, so the
    /// model never runs while the service is locked.
    pub(super) fn drive(&mut self, mut progress: Progress, notify: Notify<'_>) -> CommandOutcome {
        loop {
            progress = match progress {
                Progress::Done(outcome) => return outcome,
                Progress::Think(ticket) => {
                    let started = std::time::Instant::now();
                    let result = ticket.brain.decide(&ticket.request);
                    let ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
                    self.complete_brain(ticket.id, result, ms, notify)
                }
                Progress::Step(report) => self.advance_plan(report.id, notify),
            };
        }
    }

    // ── Conversational deterministic tier ────────────────────────────────

    /// Compound commands: a plan when two or more parts each resolve
    /// through the trusted tiers; `None` otherwise.
    fn compound(&self, text: &str, now: u64) -> Option<Resolved> {
        let parts = route::split_parts(text)?;
        {
            let mut calls: Vec<ToolCall> = Vec::new();
            let mut skipped = false;
            for p in &parts {
                let part = p.text.as_str();
                if route::has_reference(part) {
                    // "…y después ciérralo": the previous part's application.
                    let previous = calls.iter().rev().find_map(|c| self.call_app(c));
                    let (Some(app), Some(action)) = (previous, route::reference_action(part))
                    else {
                        return None;
                    };
                    calls.push(app_call(action, &app, CallOrigin::User)?);
                    continue;
                }
                let interpretation = self.understanding.interpret_deterministic(
                    &Utterance::typed(part),
                    &self.dialogue,
                    now,
                );
                match interpretation.intent {
                    // A part's application must be a trusted catalog entry
                    // (not the user's raw words sent on to "not found").
                    Intent::UseTool(call)
                        if !call.input.get("application").is_some_and(|a| a.is_string())
                            || self.call_app(&call).is_some() =>
                    {
                        calls.push(call);
                    }
                    // Only a part made of fillers ("Oye,", "OK SERSHI,") is no
                    // part; anything else unresolved means this is not a
                    // compound command the trusted tiers understand.
                    _ if is_filler(&p.own) => {}
                    // Chatter with no action verb and no application name
                    // ("ya terminé" — even with a verb carried into it):
                    // skipped, never acted on.
                    _ if !route::has_action_verb(&p.own) && self.named_apps(&p.own).is_empty() => {
                        skipped = true;
                    }
                    // A whole sentence the trusted tiers do not understand
                    // is preamble ("Necesito revisar unas cosas."): skipped,
                    // never acted on; the actions come from the parts that
                    // resolve.
                    _ if p.own.trim_end().ends_with(['.', '!']) => {
                        skipped = true;
                    }
                    _ => return None,
                }
            }
            match calls.len() {
                0 => None,
                // One action plus chatter ("Cierra el Word, ya terminé").
                1 if skipped => calls
                    .pop()
                    .map(|c| Resolved::Intent(Intent::UseTool(c), None)),
                1 => None,
                n if n > MAX_PLAN_STEPS => Some(Resolved::TooLong),
                _ => Some(Resolved::Plan(calls)),
            }
        }
    }

    /// References ("ciérralo") and elliptical follow-ups ("ahora
    /// PowerPoint"), resolved from the session context.
    fn conversational(&self, text: &str, now: u64) -> Option<Resolved> {
        // References: "ciérralo", "close that", "ábrelo otra vez".
        if route::has_reference(text) {
            let action = route::reference_action(text).or_else(|| {
                // "Hazlo de nuevo": repeat what happened last.
                self.conversation
                    .entities
                    .apps(now)
                    .next()
                    .map(|a| match a.event {
                        AppEvent::CloseRequested => AppAction::Close,
                        AppEvent::Opened | AppEvent::Mentioned => AppAction::Open,
                    })
            })?;
            let referent = self.conversation.entities.referent(now);
            if referent == Referent::None {
                // "I'm done with PowerPoint, please close it": the one
                // application the request itself names.
                let named = self.named_apps(text);
                return match named.as_slice() {
                    [app] => Some(Resolved::Intent(
                        Intent::UseTool(app_call(action, app, CallOrigin::User)?),
                        Some(UnderstoodAs {
                            action,
                            application: app.clone(),
                        }),
                    )),
                    _ => None,
                };
            }
            return match referent {
                Referent::None => None,
                Referent::One(app) => Some(Resolved::Intent(
                    Intent::UseTool(app_call(action, &app.app, CallOrigin::User)?),
                    Some(UnderstoodAs {
                        action,
                        application: app.app.clone(),
                    }),
                )),
                Referent::Ambiguous(apps) => {
                    // Oldest first: "the first one" is what was opened first.
                    let mut candidates: Vec<ApplicationSummary> =
                        apps.iter().rev().map(|a| a.app.clone()).collect();
                    if let Some(n) = ordinal(text)
                        && let Some(app) = candidates.get(n)
                    {
                        return Some(Resolved::Intent(
                            Intent::UseTool(app_call(action, app, CallOrigin::User)?),
                            Some(UnderstoodAs {
                                action,
                                application: app.clone(),
                            }),
                        ));
                    }
                    candidates.truncate(4);
                    Some(Resolved::Intent(
                        Intent::Clarify(Clarification {
                            kind: ClarificationKind::ChooseApplication,
                            action,
                            candidates,
                        }),
                        None,
                    ))
                }
            };
        }

        // Elliptical follow-ups: "ahora PowerPoint", "¿y Outlook?".
        if let Some(rest) = route::follow_up(text)
            && let Some(last) = self.conversation.entities.apps(now).next()
        {
            // "Ahora abre Word" names its own action; "¿Y Outlook?" takes
            // the last one.
            let request = if route::has_action_verb(&rest) {
                rest.clone()
            } else {
                let verb = match last.event {
                    AppEvent::CloseRequested => "cierra",
                    AppEvent::Opened | AppEvent::Mentioned => "abre",
                };
                format!("{verb} {rest}")
            };
            let interpretation = self.understanding.interpret_deterministic(
                &Utterance::typed(&request),
                &self.dialogue,
                now,
            );
            if let Intent::UseTool(call) = interpretation.intent
                && self.call_app(&call).is_some()
            {
                return Some(Resolved::Intent(
                    Intent::UseTool(call),
                    interpretation.understood,
                ));
            }
        }
        None
    }

    /// Answers a recall question from the action ledger.
    fn recall(
        &mut self,
        kind: RecallKind,
        text: &str,
        watermark: u64,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        self.reference = Some(ReferenceSource::Ledger);
        let items = self.conversation.ledger.recall(kind);
        // "What did you open?" → "Excel" → "close it": the one application
        // recalled becomes the one a reference means.
        let recalled: Vec<&ApplicationSummary> = items
            .iter()
            .filter(|i| i.result == ActionResult::Succeeded)
            .filter_map(|i| i.application.as_ref())
            .collect();
        if let [app] = recalled.as_slice() {
            let now = (self.clock)();
            let event = match kind {
                RecallKind::Closed => AppEvent::CloseRequested,
                _ => AppEvent::Opened,
            };
            if self
                .conversation
                .entities
                .apps(now)
                .next()
                .is_none_or(|a| a.app.id != app.id)
            {
                self.conversation.entities.record_app(
                    (*app).clone(),
                    event,
                    now,
                    self.conversation.request,
                );
            }
        }
        self.transition(AssistantEvent::Completed, notify);
        let reply = recall_reply(kind, &items);
        let outcome = CommandOutcome {
            detail: Some(OutcomeDetail::Recall {
                recall: kind,
                items,
            }),
            ..CommandOutcome::new(CommandStatus::Answered, reply)
        };
        let trace = BrainTrace {
            route: UnderstandingRoute::AgentBrain,
            brain: Some(BrainUse::Deterministic),
            model_ms: None,
            prompt_version: BRAIN_PROMPT_VERSION,
            steps: 0,
        };
        self.finish(outcome, text, Some(trace), watermark, notify)
    }

    /// "Los dos" / "both" answering an open question about exactly two
    /// trusted candidates ("todos"/"them": all of them, at most a plan).
    fn plural_answer(&self, text: &str, now: u64) -> Option<(Vec<ToolCall>, ReferenceSource)> {
        let plural = route::plural(text)?;
        let pending = self.dialogue.pending(now)?;
        let c = &pending.clarification;
        if !matches!(
            c.kind,
            ClarificationKind::ChooseApplication | ClarificationKind::MultipleTargets
        ) {
            return None;
        }
        let n = c.candidates.len();
        let fits = match plural {
            route::Plural::Two => n == 2,
            route::Plural::All => (2..=MAX_PLAN_STEPS).contains(&n),
        };
        if !fits {
            return None;
        }
        let calls = c
            .candidates
            .iter()
            .map(|app| app_call(c.action, app, CallOrigin::User))
            .collect::<Option<Vec<_>>>()?;
        Some((calls, ReferenceSource::Clarification))
    }

    /// "Close both" / "ciérralos" after acting on several applications
    /// together: the applications of that group, oldest first. "Both"
    /// needs exactly two.
    fn plural_reference(&self, text: &str, now: u64) -> Option<(Vec<ToolCall>, ReferenceSource)> {
        let plural = route::plural(text)?;
        let action = route::reference_action(text)?;
        let group: Vec<ApplicationSummary> = match self.conversation.entities.referent(now) {
            Referent::Ambiguous(apps) => apps.iter().rev().map(|a| a.app.clone()).collect(),
            Referent::One(_) | Referent::None => return None,
        };
        let fits = match plural {
            route::Plural::Two => group.len() == 2,
            route::Plural::All => (2..=MAX_PLAN_STEPS).contains(&group.len()),
        };
        if !fits {
            return None;
        }
        let calls = group
            .iter()
            .map(|app| app_call(action, app, CallOrigin::User))
            .collect::<Option<Vec<_>>>()?;
        Some((calls, ReferenceSource::Entities))
    }

    /// Trusted applications whose name appears in `text` as whole words.
    fn named_apps(&self, text: &str) -> Vec<ApplicationSummary> {
        let key = format!(" {} ", crate::apps::normalize::normalize(text));
        let mut found: Vec<ApplicationSummary> = Vec::new();
        for entry in self.understanding.catalog_names() {
            let named = entry
                .names
                .iter()
                .any(|n| n.len() >= 3 && key.contains(&format!(" {n} ")));
            if named && !found.iter().any(|a| a.id == entry.application.id) {
                found.push(entry.application);
            }
        }
        found
    }

    /// Whether Gate 3C resolves `u` only through a system keyword (no
    /// application): such a question may still be conversation.
    fn keyword_only(&self, u: &Utterance<'_>, now: u64) -> bool {
        let interpretation = self
            .understanding
            .interpret_deterministic(u, &self.dialogue, now);
        interpretation.trace.tier == crate::understanding::ResolutionTier::Keyword
            && matches!(&interpretation.intent, Intent::UseTool(call) if call.input.get("application").is_none())
    }

    /// The trusted application a call names, if it is a catalog entry.
    pub(super) fn call_app(&self, call: &ToolCall) -> Option<ApplicationSummary> {
        let id = call.input.get("application")?.as_str()?;
        self.understanding
            .catalog_names()
            .into_iter()
            .map(|n| n.application)
            .find(|a| a.id == id)
    }

    // ── Brain model ───────────────────────────────────────────────────────

    fn think(
        &mut self,
        u: &Utterance<'_>,
        watermark: u64,
        had_question: bool,
        now: u64,
    ) -> Progress {
        let Some(brain) = self.brain.clone() else {
            // Defensive: callers check first.
            let outcome =
                CommandOutcome::new(CommandStatus::NotUnderstood, "I didn't understand that.");
            return Progress::Done(outcome);
        };
        self.ticket_seq += 1;
        let offered = self.offered_apps(u, now);
        let handle_of = |app: &ApplicationSummary| {
            offered
                .iter()
                .position(|o| o.id == app.id)
                .map(crate::brain::contract::handle)
        };
        let mut context = self.conversation.entities.lines(now, handle_of);
        // What did not work, from the ledger: the brain never reports a
        // failed action as done (successes are the entity lines above).
        for entry in self
            .conversation
            .ledger
            .entries()
            .filter(|e| e.result != ActionResult::Succeeded)
            .filter(|e| now.saturating_sub(e.at_ms) < crate::brain::context::ENTITY_TTL_MS)
            .take(2)
        {
            let what = entry
                .application
                .as_ref()
                .map_or_else(String::new, |a| format!(" {}", a.display_name));
            let verb = match entry.action {
                StepAction::Open => "open",
                StepAction::Close => "close",
                StepAction::Memory => "check memory",
                StepAction::Cpu => "check the processor",
                StepAction::SystemInfo => "show system information",
            };
            context.push(format!("tried to {verb}{what}: it did not work"));
        }
        for turn in self.dialogue.recent(now) {
            if let Some(summary) = &turn.summary {
                context.push(format!(
                    "earlier the user said \"{}\" and SERSHI {summary}",
                    turn.text
                ));
            }
        }
        if let Some(q) = &self.brain_question {
            let options: Vec<String> = q
                .options
                .iter()
                .filter_map(|o| handle_of(o).map(|h| format!("{h} ({})", o.display_name)))
                .collect();
            context.push(format!(
                "you just asked: \"{}\" options: {}",
                q.message,
                options.join(", ")
            ));
        }
        context.truncate(8);
        let request = BrainRequest {
            text: u.text.to_owned(),
            response_language: self.response_language.clone(),
            apps: offered.iter().map(|a| a.display_name.clone()).collect(),
            context,
            capabilities: manifest(self.executor.registry().definitions()),
            unavailable: self.unavailable.clone(),
        };
        self.brain_question = None;
        self.thinking = Some(Thinking {
            id: self.ticket_seq,
            text: u.text.to_owned(),
            asr_confidence: u.asr_confidence,
            language: u.language.map(str::to_owned),
            source: u.source,
            offered,
            watermark,
            had_question,
        });
        Progress::Think(BrainTicket {
            id: self.ticket_seq,
            request,
            brain,
        })
    }

    /// Applications the brain may name: the open question's options, the
    /// conversation's recent ones, Gate 3C's candidates for this text, then
    /// well-known installed applications. Names only; no paths.
    fn offered_apps(&self, u: &Utterance<'_>, now: u64) -> Vec<ApplicationSummary> {
        let mut out: Vec<ApplicationSummary> = Vec::new();
        let push = |app: ApplicationSummary, out: &mut Vec<ApplicationSummary>| {
            if out.len() < MAX_OFFERED && !out.iter().any(|a| a.id == app.id) {
                out.push(app);
            }
        };
        if let Some(q) = &self.brain_question {
            for o in &q.options {
                push(o.clone(), &mut out);
            }
        }
        for a in self.conversation.entities.apps(now) {
            push(a.app.clone(), &mut out);
        }
        if let Some((_, options)) = self.understanding.semantic_request(u, &self.dialogue, now) {
            for o in options {
                push(o, &mut out);
            }
        }
        let names = self.understanding.catalog_names();
        for common in crate::voice::latency::COMMON_APPS {
            let key = common.to_lowercase();
            if let Some(entry) = names
                .iter()
                .find(|n| n.application.display_name.to_lowercase() == key)
            {
                push(entry.application.clone(), &mut out);
            }
        }
        out
    }

    /// The brain's decision for `ticket`. A ticket that is no longer current
    /// (the request was cancelled, dismissed or replaced) is discarded:
    /// nothing it says can act.
    pub fn complete_brain(
        &mut self,
        ticket: u64,
        result: Result<BrainDecision, BrainError>,
        model_ms: u32,
        notify: Notify<'_>,
    ) -> Progress {
        let Some(thinking) = self.thinking.take().filter(|t| t.id == ticket) else {
            return Progress::Done(CommandOutcome::new(
                CommandStatus::Cancelled,
                "That request was cancelled. Nothing was changed.",
            ));
        };
        let now = (self.clock)();
        let text = thinking.text.clone();
        let mut trace = BrainTrace {
            route: UnderstandingRoute::AgentBrain,
            brain: Some(BrainUse::Model),
            model_ms: Some(model_ms),
            prompt_version: BRAIN_PROMPT_VERSION,
            steps: 0,
        };
        let decision = match result {
            Ok(d) => d,
            Err(error) => {
                // The brain failed: Gate 3C answers instead (graceful
                // degradation; nothing from the brain is used).
                trace.brain = Some(if error == BrainError::InvalidOutput {
                    BrainUse::Rejected
                } else {
                    BrainUse::Failed
                });
                let u = Utterance {
                    text: &thinking.text,
                    source: thinking.source,
                    asr_confidence: thinking.asr_confidence,
                    language: thinking.language.as_deref(),
                };
                let interpretation = self.interpret_gate3c(&u, now);
                let understood = interpretation.understood.clone();
                let understanding = interpretation.trace.clone();
                let mut outcome =
                    self.act_on(interpretation.intent, None, thinking.had_question, notify);
                outcome.understood = understood;
                outcome.understanding = Some(understanding);
                return Progress::Done(self.finish(
                    outcome,
                    &text,
                    Some(trace),
                    thinking.watermark,
                    notify,
                ));
            }
        };
        let offered = |i: usize| thinking.offered.get(i).cloned();
        match decision {
            BrainDecision::Answer { message } => {
                self.transition(AssistantEvent::Completed, notify);
                let outcome = CommandOutcome {
                    detail: Some(OutcomeDetail::BrainAnswer {
                        message: message.clone(),
                    }),
                    ..CommandOutcome::new(CommandStatus::Answered, message)
                };
                Progress::Done(self.finish(outcome, &text, Some(trace), thinking.watermark, notify))
            }
            BrainDecision::Clarify { message, options } => {
                let options: Vec<ApplicationSummary> =
                    options.into_iter().filter_map(offered).collect();
                self.brain_question = Some(BrainQuestion {
                    message: message.clone(),
                    options: options.clone(),
                    expires_at: now + BRAIN_QUESTION_TTL_MS,
                });
                self.record(NewActivity::new(
                    ActivityKind::ClarificationRequested,
                    "Asked a question",
                ));
                self.transition(AssistantEvent::ClarificationRequested, notify);
                let outcome = CommandOutcome {
                    detail: Some(OutcomeDetail::BrainQuestion {
                        message: message.clone(),
                        options,
                    }),
                    ..CommandOutcome::new(CommandStatus::NeedsClarification, message)
                };
                Progress::Done(self.finish(outcome, &text, Some(trace), thinking.watermark, notify))
            }
            BrainDecision::Act { steps, .. } => {
                let capabilities = manifest(self.executor.registry().definitions());
                let mut calls = Vec::with_capacity(steps.len());
                for step in &steps {
                    // Validated by the parser; checked again here against
                    // the registry and the offered applications.
                    let Some(capability) = capabilities.iter().find(|c| c.name == step.capability)
                    else {
                        trace.brain = Some(BrainUse::Rejected);
                        return Progress::Done(self.brain_rejected(
                            &text,
                            trace,
                            thinking.watermark,
                            notify,
                        ));
                    };
                    let input = match (capability.takes_app, step.app.and_then(offered)) {
                        (true, Some(app)) => serde_json::json!({ "application": app.id }),
                        (false, None) if step.app.is_none() => Value::Null,
                        _ => {
                            trace.brain = Some(BrainUse::Rejected);
                            return Progress::Done(self.brain_rejected(
                                &text,
                                trace,
                                thinking.watermark,
                                notify,
                            ));
                        }
                    };
                    // Agent origin: policy stays conservative, and every
                    // sensitive step needs the trusted confirmation.
                    let Ok(tool_id) = ToolId::new(capability.tool_id) else {
                        trace.brain = Some(BrainUse::Rejected);
                        return Progress::Done(self.brain_rejected(
                            &text,
                            trace,
                            thinking.watermark,
                            notify,
                        ));
                    };
                    calls.push(ToolCall::new(tool_id, input, CallOrigin::Agent));
                }
                trace.steps = calls.len();
                if calls.len() == 1 {
                    let call = calls.remove(0);
                    let understood = self.call_app(&call).map(|app| UnderstoodAs {
                        action: if call.tool_id.as_str() == CLOSE_APPLICATION {
                            AppAction::Close
                        } else {
                            AppAction::Open
                        },
                        application: app,
                    });
                    let mut outcome = self.act_on(Intent::UseTool(call), None, false, notify);
                    outcome.understood = understood;
                    return Progress::Done(self.finish(
                        outcome,
                        &text,
                        Some(trace),
                        thinking.watermark,
                        notify,
                    ));
                }
                self.start_plan(calls, trace, &text, thinking.watermark, notify)
            }
            BrainDecision::Cancel => {
                let outcome = self.act_on(Intent::Cancel, None, thinking.had_question, notify);
                Progress::Done(self.finish(outcome, &text, Some(trace), thinking.watermark, notify))
            }
            BrainDecision::Unknown => {
                let outcome = self.act_on(Intent::NotUnderstood, None, false, notify);
                Progress::Done(self.finish(outcome, &text, Some(trace), thinking.watermark, notify))
            }
        }
    }

    fn brain_rejected(
        &mut self,
        text: &str,
        trace: BrainTrace,
        watermark: u64,
        notify: Notify<'_>,
    ) -> CommandOutcome {
        let outcome = self.act_on(Intent::NotUnderstood, None, false, notify);
        self.finish(outcome, text, Some(trace), watermark, notify)
    }

    /// The brain decision in flight, if any, is abandoned (Escape, a new
    /// request, SERSHI hidden). Its result will be discarded.
    pub fn abandon_brain(&mut self, notify: Notify<'_>) -> bool {
        if self.thinking.take().is_none() {
            return false;
        }
        if self.machine.state() == AssistantState::Thinking {
            self.transition(AssistantEvent::Dismiss, notify);
        }
        true
    }

    // ── Plans ─────────────────────────────────────────────────────────────

    fn start_plan(
        &mut self,
        calls: Vec<ToolCall>,
        mut trace: BrainTrace,
        text: &str,
        watermark: u64,
        notify: Notify<'_>,
    ) -> Progress {
        if calls.len() > MAX_PLAN_STEPS {
            return Progress::Done(self.plan_too_long(text, watermark, notify));
        }
        trace.steps = calls.len();
        self.plan_seq += 1;
        // A close of an application the plan opens depends on that open.
        let depends: Vec<Option<usize>> = calls
            .iter()
            .enumerate()
            .map(|(i, c)| {
                (c.tool_id.as_str() == CLOSE_APPLICATION)
                    .then(|| {
                        calls[..i].iter().rposition(|p| {
                            p.tool_id.as_str() == OPEN_APPLICATION && p.input == c.input
                        })
                    })
                    .flatten()
            })
            .collect();
        let steps = calls
            .iter()
            .map(|c| PlanStepReport {
                action: step_action(&c.tool_id),
                application: self.call_app(c),
                status: StepStatus::Pending,
                data: None,
            })
            .collect();
        self.plan = Some(ActivePlan {
            id: self.plan_seq,
            calls,
            steps,
            depends,
            cursor: 0,
            awaiting: None,
            trace,
        });
        self.record(NewActivity::new(
            ActivityKind::PlanStarted,
            format!("Started a {}-step plan", trace.steps),
        ));
        self.dialogue
            .record((self.clock)(), text, Some("started a plan".to_owned()));
        self.flush_activity(watermark, notify);
        let report = self.plan.as_ref().map(ActivePlan::report);
        match report {
            Some(report) => {
                notify(ServiceEvent::Plan(report.clone()));
                Progress::Step(report)
            }
            None => Progress::Done(CommandOutcome::new(CommandStatus::Failed, "No plan.")),
        }
    }

    /// Runs the next step of plan `id`. A plan that is no longer current
    /// (cancelled, replaced, finished) runs nothing.
    pub fn advance_plan(&mut self, id: u64, notify: Notify<'_>) -> Progress {
        let watermark = self.watermark();
        let Some(plan) = self
            .plan
            .as_mut()
            .filter(|p| p.id == id && p.awaiting.is_none())
        else {
            return Progress::Done(CommandOutcome::new(
                CommandStatus::Cancelled,
                "That plan is no longer running. Nothing else was changed.",
            ));
        };
        let i = plan.cursor;
        if i >= plan.calls.len() {
            return Progress::Done(self.finish_plan(watermark, notify));
        }
        // A step whose prerequisite did not succeed is skipped.
        if let Some(d) = plan.depends[i]
            && plan.steps[d].status != StepStatus::Completed
        {
            plan.steps[i].status = StepStatus::Skipped;
            plan.cursor += 1;
            let report = plan.report();
            notify(ServiceEvent::Plan(report.clone()));
            return Progress::Step(report);
        }
        plan.steps[i].status = StepStatus::Running;
        let call = plan.calls[i].clone();
        notify(ServiceEvent::Plan(plan.report()));

        if self.machine.state() == AssistantState::Thinking {
            self.transition(AssistantEvent::PlanStarted, notify);
        }
        let outcome = self.run_call(&call, true, notify);
        let status = match outcome.status {
            CommandStatus::Completed => StepStatus::Completed,
            CommandStatus::NeedsConfirmation => StepStatus::NeedsConfirmation,
            CommandStatus::Unresolved => StepStatus::Unresolved,
            _ => StepStatus::Failed,
        };
        let Some(plan) = self.plan.as_mut() else {
            return Progress::Done(outcome);
        };
        plan.steps[i].status = status;
        plan.steps[i].data = outcome.data.clone();
        if status == StepStatus::NeedsConfirmation {
            // Paused in the trusted confirmation window; this approval will
            // cover this step only.
            plan.awaiting = Some(i);
            let report = plan.report();
            let trace = plan.trace;
            notify(ServiceEvent::Plan(report.clone()));
            self.flush_activity(watermark, notify);
            return Progress::Done(CommandOutcome {
                plan: Some(report),
                brain: Some(trace),
                session: Some(Box::new(self.session_trace())),
                ..outcome
            });
        }
        plan.cursor += 1;
        let report = plan.report();
        let more = plan.cursor < plan.calls.len();
        notify(ServiceEvent::Plan(report.clone()));
        self.flush_activity(watermark, notify);
        if more {
            Progress::Step(report)
        } else {
            Progress::Done(self.finish_plan(watermark, notify))
        }
    }

    /// After the trusted surface decided on a plan's step: records its
    /// result and, if approved, lets the plan continue. Declined or expired:
    /// the rest of the plan is cancelled.
    pub(super) fn plan_after_decision(
        &mut self,
        outcome: &CommandOutcome,
        approved: bool,
        notify: Notify<'_>,
    ) -> Option<Progress> {
        let plan = self.plan.as_mut()?;
        let i = plan.awaiting.take()?;
        if !approved {
            for step in &mut plan.steps[i..] {
                step.status = StepStatus::Cancelled;
            }
            plan.cursor = plan.calls.len();
            let report = plan.report();
            self.plan = None;
            self.record(NewActivity::new(
                ActivityKind::PlanCancelled,
                "Plan cancelled",
            ));
            notify(ServiceEvent::Plan(report.clone()));
            return Some(Progress::Done(CommandOutcome {
                plan: Some(report),
                ..outcome.clone()
            }));
        }
        plan.steps[i].status = match outcome.status {
            CommandStatus::Completed => StepStatus::Completed,
            CommandStatus::Unresolved => StepStatus::Unresolved,
            _ => StepStatus::Failed,
        };
        plan.steps[i].data = outcome.data.clone();
        plan.cursor = i + 1;
        let report = plan.report();
        notify(ServiceEvent::Plan(report.clone()));
        if plan.cursor < plan.calls.len() {
            Some(Progress::Step(report))
        } else {
            let watermark = self.watermark();
            Some(Progress::Done(self.finish_plan(watermark, notify)))
        }
    }

    fn finish_plan(&mut self, watermark: u64, notify: Notify<'_>) -> CommandOutcome {
        let Some(plan) = self.plan.take() else {
            return CommandOutcome::new(CommandStatus::Cancelled, "No plan.");
        };
        let report = PlanReport {
            done: true,
            ..plan.report()
        };
        let done = report
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count();
        let (status, event) = if done == report.steps.len() {
            (CommandStatus::Completed, AssistantEvent::Completed)
        } else if done > 0 {
            (CommandStatus::Partial, AssistantEvent::AttentionNeeded)
        } else {
            (CommandStatus::Failed, AssistantEvent::Failed)
        };
        if self.machine.state().is_busy() {
            self.transition(event, notify);
        }
        self.record(NewActivity::new(
            ActivityKind::PlanFinished,
            format!("Plan finished: {done} of {} steps done", report.steps.len()),
        ));
        self.flush_activity(watermark, notify);
        notify(ServiceEvent::Plan(report.clone()));
        CommandOutcome {
            plan: Some(report),
            brain: Some(plan.trace),
            session: Some(Box::new(self.session_trace())),
            ..CommandOutcome::new(
                status,
                format!("Plan finished: {done} of {} steps done.", plan.steps.len()),
            )
        }
    }

    /// Cancels a running plan (new request, dismissal). Nothing else runs.
    pub(super) fn cancel_plan(&mut self) -> bool {
        let Some(mut plan) = self.plan.take() else {
            return false;
        };
        for step in &mut plan.steps {
            if matches!(
                step.status,
                StepStatus::Pending | StepStatus::Running | StepStatus::NeedsConfirmation
            ) {
                step.status = StepStatus::Cancelled;
            }
        }
        self.record(NewActivity::new(
            ActivityKind::PlanCancelled,
            "Plan cancelled",
        ));
        true
    }

    fn plan_too_long(&mut self, text: &str, watermark: u64, notify: Notify<'_>) -> CommandOutcome {
        self.transition(AssistantEvent::AttentionNeeded, notify);
        let outcome = CommandOutcome {
            detail: Some(OutcomeDetail::PlanTooLong {
                max_steps: MAX_PLAN_STEPS,
            }),
            ..CommandOutcome::new(
                CommandStatus::NotUnderstood,
                format!("That's more than {MAX_PLAN_STEPS} steps. Please ask in smaller parts."),
            )
        };
        self.finish(outcome, text, None, watermark, notify)
    }

    // ── Session entities ──────────────────────────────────────────────────

    /// Records what a tool result tells the conversation (trusted data only).
    pub(super) fn note_result(&mut self, call: &ToolCall, outcome: &CommandOutcome) {
        let now = (self.clock)();
        let data = outcome.data.as_ref();
        // Only trusted catalog entries become context: the id in the
        // tool's data is looked up, never deserialized into an entity.
        let app_in_data = || {
            let id = data?.get("application")?.get("id")?.as_str()?;
            self.understanding
                .catalog_names()
                .into_iter()
                .map(|n| n.application)
                .find(|a| a.id == id)
        };
        match (call.tool_id.as_str(), outcome.status) {
            (OPEN_APPLICATION, CommandStatus::Completed) => {
                if let Some(app) = app_in_data() {
                    let request = self.conversation.request;
                    self.conversation
                        .entities
                        .record_app(app, AppEvent::Opened, now, request);
                }
            }
            (CLOSE_APPLICATION, CommandStatus::Completed) => {
                if let Some(app) = app_in_data() {
                    let request = self.conversation.request;
                    self.conversation.entities.record_app(
                        app,
                        AppEvent::CloseRequested,
                        now,
                        request,
                    );
                }
            }
            (CLOSE_APPLICATION, CommandStatus::NeedsConfirmation) => {
                if let Some(ConfirmationSubject::Application { application }) =
                    self.confirmations.current().and_then(|c| c.subject.clone())
                {
                    self.conversation.entities.record_app(
                        application,
                        AppEvent::CloseRequested,
                        now,
                        self.conversation.request,
                    );
                }
            }
            ("system.get_memory", CommandStatus::Completed) => {
                let n = |k: &str| data.and_then(|d| d.get(k)).and_then(Value::as_u64);
                if let (Some(used), Some(total)) = (n("usedBytes"), n("totalBytes")) {
                    self.conversation.entities.record_fact(
                        SystemFact::Memory {
                            used_bytes: used,
                            total_bytes: total,
                        },
                        now,
                    );
                }
            }
            ("system.get_cpu", CommandStatus::Completed) => {
                let usage = data
                    .and_then(|d| d.get("usagePercent"))
                    .and_then(Value::as_f64)
                    .map(|v| v as f32);
                self.conversation.entities.record_fact(
                    SystemFact::Cpu {
                        usage_percent: usage,
                    },
                    now,
                );
            }
            ("system.get_info", CommandStatus::Completed) => {
                self.conversation
                    .entities
                    .record_fact(SystemFact::Info, now);
            }
            _ => {}
        }
    }
}

/// A plural shortcut is never taken over tampering words or while the
/// brain's own question is open (the brain owns that exchange).
fn tampering_or_brain_question(text: &str, brain_question: bool) -> bool {
    brain_question || route::tampering(text)
}

/// Canonical English for a recall answer (surfaces phrase it themselves
/// from the structured items).
fn recall_reply(kind: RecallKind, items: &[RecallItem]) -> String {
    let names = |want: ActionResult| -> Vec<String> {
        items
            .iter()
            .filter(|i| (i.result == ActionResult::Succeeded) == (want == ActionResult::Succeeded))
            .map(|i| {
                i.application
                    .as_ref()
                    .map_or_else(|| "that".to_owned(), |a| a.display_name.clone())
            })
            .collect()
    };
    let done = names(ActionResult::Succeeded);
    let failed = names(ActionResult::Failed);
    let (verb, none) = match kind {
        RecallKind::Opened => (
            "opened",
            "I haven't opened an application in this conversation.",
        ),
        RecallKind::Closed => (
            "closed",
            "I haven't closed an application in this conversation.",
        ),
        RecallKind::Did => ("did", "I haven't done anything in this conversation yet."),
    };
    if items.is_empty() {
        return none.to_owned();
    }
    if kind == RecallKind::Did {
        return format!(
            "{} action(s): {} done, {} not done.",
            items.len(),
            done.len(),
            failed.len()
        );
    }
    let mut out = String::new();
    if !done.is_empty() {
        out.push_str(&format!("I {verb} {}.", done.join(" and ")));
    }
    if !failed.is_empty() {
        let attempt = if kind == RecallKind::Opened {
            "open"
        } else {
            "close"
        };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!("I couldn't {attempt} {}.", failed.join(" or ")));
    }
    out
}
