//! Payloads exchanged with surfaces: requests in, outcomes out.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::activity::ActivityEntry;
use crate::assistant::AssistantSnapshot;
use crate::confirmation::ConfirmationId;
use crate::ids::ToolId;
use crate::intent::AnswerTopic;
use crate::policy::DenialReason;

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
    /// A tool ran successfully.
    Completed,
    /// SERSHI answered without acting.
    Answered,
    NeedsConfirmation,
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
}

pub type Clock = fn() -> u64;

/// Push-to-talk was refused because SERSHI is working, speaking or already
/// handling voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceBusy;
