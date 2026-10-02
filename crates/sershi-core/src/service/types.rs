//! Payloads exchanged with surfaces: requests in, outcomes out.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::activity::ActivityEntry;
use crate::apps::ApplicationSummary;
use crate::assistant::AssistantSnapshot;
use crate::brain::route::UnderstandingRoute;
use crate::confirmation::ConfirmationId;
use crate::ids::ToolId;
use crate::intent::AnswerTopic;
use crate::policy::DenialReason;
use crate::understanding::{Clarification, UnderstandingTrace, UnderstoodAs};

/// Longest command accepted over IPC.
pub const MAX_COMMAND_CHARS: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandRequest {
    pub text: String,
}

/// The human's answer on the trusted confirmation surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationChoice {
    Approve,
    Cancel,
}

/// Everything a surface can send about a confirmation: which one, and the
/// decision. The tool, its input, the target, the risk and the permission
/// are all resolved from the core's stored action; any other field is
/// rejected (`deny_unknown_fields`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmationDecision {
    pub confirmation_id: ConfirmationId,
    pub decision: ConfirmationChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum CommandStatus {
    /// A tool ran successfully (for a plan: every step did).
    Completed,
    /// A plan finished with some steps done and others not (failed,
    /// skipped, unresolved). The plan report says which.
    Partial,
    /// SERSHI answered without acting.
    Answered,
    NeedsConfirmation,
    /// SERSHI asked which application was meant; nothing ran.
    NeedsClarification,
    Denied,
    /// Understood, but the capability is not built yet.
    Unavailable,
    /// The tool ran into an expected negative result (not found, ambiguous,
    /// not running…). `data` has the details.
    Unresolved,
    NotUnderstood,
    Failed,
    /// The user cancelled a pending confirmation.
    Cancelled,
    /// A pending confirmation expired, or its target changed.
    Expired,
    /// The request itself was invalid (empty, too long, busy, unknown
    /// confirmation).
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum RejectionReason {
    Empty,
    TooLong,
    Busy,
    /// The confirmation id is unknown, already decided or replaced.
    UnknownConfirmation,
}

/// Structured facts behind a reply, so each surface can phrase it in the
/// user's interface language instead of relaying the English `reply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum OutcomeDetail {
    Answer {
        topic: AnswerTopic,
    },
    Unavailable {
        capability: String,
        milestone: String,
    },
    Denied {
        reason: DenialReason,
    },
    Rejected {
        reason: RejectionReason,
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        max_chars: usize,
    },
    Clarification {
        clarification: Clarification,
    },
    /// The Agent Brain's conversational reply (no action). The message is
    /// the model's text in the response language: shown as SERSHI's words,
    /// never as a report of what happened.
    BrainAnswer {
        message: String,
    },
    /// The Agent Brain asked a question; `options` are trusted
    /// applications (names, never paths) the user can pick.
    BrainQuestion {
        message: String,
        options: Vec<ApplicationSummary>,
    },
    /// A plan longer than SERSHI runs in one go ([`crate::brain::MAX_PLAN_STEPS`]).
    PlanTooLong {
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        max_steps: usize,
    },
}

/// What one plan step does (no paths, ids or arguments beyond the trusted
/// application).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum StepAction {
    Open,
    Close,
    Memory,
    Cpu,
    SystemInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    /// Waiting in the trusted confirmation window.
    NeedsConfirmation,
    /// Ran into an expected negative result (not found, not running…).
    Unresolved,
    Failed,
    /// Not run: a step it depends on did not succeed.
    Skipped,
    /// Not run: the plan was cancelled, replaced or its approval declined.
    Cancelled,
}

/// One step of a plan, as surfaces show it. `data` is the tool's structured
/// result (trusted), so the final sentence is phrased from facts, never
/// from the model's words.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PlanStepReport {
    pub action: StepAction,
    pub application: Option<ApplicationSummary>,
    pub status: StepStatus,
    pub data: Option<Value>,
}

/// A bounded multi-step plan and how far it got.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PlanReport {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub id: u64,
    pub steps: Vec<PlanStepReport>,
    /// Finished: no step will run any more.
    pub done: bool,
}

/// How the brain route went (developer diagnostics; never the prompt, the
/// model's reasoning or the request text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum BrainUse {
    /// The conversational layer's deterministic tier (references, compound
    /// commands, follow-ups): no model.
    Deterministic,
    /// The brain model decided.
    Model,
    /// No brain model installed or enabled.
    NotInstalled,
    /// The model failed or timed out; Gate 3C understanding answered.
    Failed,
    /// The model's output was rejected by validation.
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct BrainTrace {
    pub route: UnderstandingRoute,
    pub brain: Option<BrainUse>,
    /// Model time, when the model ran.
    pub model_ms: Option<u32>,
    pub prompt_version: u32,
    /// Steps in the resulting plan (0 for answers and questions).
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub steps: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    pub status: CommandStatus,
    /// What SERSHI says back, in canonical English. Surfaces localise from
    /// `status`, `tool_id`, `data` and `detail`, and fall back to this text
    /// for anything they cannot phrase themselves.
    pub reply: String,
    pub detail: Option<OutcomeDetail>,
    pub tool_id: Option<ToolId>,
    pub data: Option<Value>,
    pub duration_ms: Option<u32>,
    /// Set when SERSHI acted on a repaired or inferred reading of the
    /// request ("SERSHI understood: Open Word"), so the user can see it.
    pub understood: Option<UnderstoodAs>,
    /// Developer diagnostics (transient; never stored).
    pub understanding: Option<UnderstandingTrace>,
    /// A multi-step plan and its progress.
    pub plan: Option<PlanReport>,
    /// Routing and brain diagnostics (transient; never stored).
    pub brain: Option<BrainTrace>,
}

impl CommandOutcome {
    pub(crate) fn new(status: CommandStatus, reply: impl Into<String>) -> Self {
        Self {
            status,
            reply: reply.into(),
            detail: None,
            tool_id: None,
            data: None,
            duration_ms: None,
            understood: None,
            understanding: None,
            plan: None,
            brain: None,
        }
    }

    pub(crate) fn rejected(reason: RejectionReason, reply: impl Into<String>) -> Self {
        Self {
            detail: Some(OutcomeDetail::Rejected {
                reason,
                max_chars: MAX_COMMAND_CHARS,
            }),
            ..Self::new(CommandStatus::Rejected, reply)
        }
    }
}

/// Changes the host should broadcast to every surface.
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceEvent {
    State(AssistantSnapshot),
    Activity(ActivityEntry),
    /// A plan's progress (after each step), for live display.
    Plan(PlanReport),
}

pub type Clock = fn() -> u64;

/// Push-to-talk was refused because SERSHI is working, speaking or already
/// handling voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceBusy;
