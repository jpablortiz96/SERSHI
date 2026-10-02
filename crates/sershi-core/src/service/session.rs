//! The conversation session (Gate 4.1): one authoritative, in-memory state
//! shared by typed and spoken requests, the Agent Brain, plans, references
//! and the conversation UI.
//!
//! ```text
//! ConversationSession
//!   id        changes with "New conversation"
//!   entities  recent applications and the last system fact (references)
//!   ledger    what SERSHI actually did (tool results only)
//! ```
//!
//! Together with the service's [`crate::understanding::Dialogue`] (the open
//! question, recent turns) and the plan and brain question it holds, this is
//! the whole conversation. There is no second history for voice: a spoken
//! request and a typed one read and write exactly this state.
//!
//! **The ledger is authoritative for what SERSHI did.** "What did you just
//! open?" is answered from it, deterministically — never from a model's
//! recollection — and a failed action is never remembered as a success.
//! Memory only, bounded ([`MAX_LEDGER_ENTRIES`]), never written to disk; the
//! security audit ([`crate::activity`]) is separate and survives a reset.

use std::collections::VecDeque;

use serde::Serialize;

use super::types::{CommandStatus, StepAction};
use crate::apps::ApplicationSummary;
use crate::brain::SessionContext;
use crate::ids::ToolId;
use crate::policy::Authorization;
use crate::tool::CallOrigin;

/// Most actions kept for recall (a conversation's worth; older ones are not
/// "just" anything).
pub const MAX_LEDGER_ENTRIES: usize = 32;

/// How an executed action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ActionResult {
    Succeeded,
    /// An expected negative result (not found, not running).
    Unresolved,
    Failed,
    /// It needed approval and was declined, cancelled or expired: nothing
    /// ran (Gate 4.1.1).
    Cancelled,
}

impl ActionResult {
    pub fn from_status(status: CommandStatus) -> Option<Self> {
        match status {
            CommandStatus::Completed => Some(Self::Succeeded),
            CommandStatus::Unresolved => Some(Self::Unresolved),
            CommandStatus::Failed => Some(Self::Failed),
            CommandStatus::Cancelled | CommandStatus::Expired => Some(Self::Cancelled),
            // Nothing ran: confirmation pending, denied, cancelled, expired…
            _ => None,
        }
    }
}

/// One real tool execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub seq: u64,
    pub at_ms: u64,
    /// The request (turn) it belongs to; a plan's steps share one.
    pub request: u64,
    pub tool_id: ToolId,
    pub action: StepAction,
    /// The trusted catalog entry acted on, if any.
    pub application: Option<ApplicationSummary>,
    pub result: ActionResult,
    pub origin: CallOrigin,
    pub plan: Option<u64>,
    /// Why it was allowed to run (successful actions).
    pub authorization: Option<Authorization>,
}

/// What SERSHI actually did in this conversation, newest first.
#[derive(Debug, Default, Clone)]
pub struct ActionLedger {
    entries: VecDeque<LedgerEntry>,
    seq: u64,
}

impl ActionLedger {
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        at_ms: u64,
        request: u64,
        tool_id: ToolId,
        action: StepAction,
        application: Option<ApplicationSummary>,
        result: ActionResult,
        origin: CallOrigin,
        plan: Option<u64>,
        authorization: Option<Authorization>,
    ) -> u64 {
        self.seq += 1;
        self.entries.push_front(LedgerEntry {
            seq: self.seq,
            at_ms,
            request,
            tool_id,
            action,
            application,
            result,
            origin,
            plan,
            authorization,
        });
        self.entries.truncate(MAX_LEDGER_ENTRIES);
        self.seq
    }

    /// Newest first.
    pub fn entries(&self) -> impl Iterator<Item = &LedgerEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The most recent successful action.
    pub fn last_success(&self) -> Option<&LedgerEntry> {
        self.entries
            .iter()
            .find(|e| e.result == ActionResult::Succeeded)
    }

    /// Answers "what did you just open / close / do?" from real results.
    pub fn recall(&self, kind: RecallKind) -> Vec<RecallItem> {
        let item = |e: &LedgerEntry| RecallItem {
            action: e.action,
            application: e.application.clone(),
            result: e.result,
        };
        match kind {
            RecallKind::Did => {
                // Everything the latest request did, in the order it ran.
                let Some(latest) = self.entries.front().map(|e| e.request) else {
                    return Vec::new();
                };
                let mut items: Vec<RecallItem> = self
                    .entries
                    .iter()
                    .take_while(|e| e.request == latest)
                    .map(item)
                    .collect();
                items.reverse();
                items
            }
            RecallKind::Opened | RecallKind::Closed => {
                let wanted = if kind == RecallKind::Opened {
                    StepAction::Open
                } else {
                    StepAction::Close
                };
                let of_kind = |e: &&LedgerEntry| e.action == wanted;
                // The latest request that succeeded at it, all its
                // successes (a plan may have opened two)…
                let success = self
                    .entries
                    .iter()
                    .filter(of_kind)
                    .find(|e| e.result == ActionResult::Succeeded);
                let mut items: Vec<RecallItem> = match success {
                    Some(s) => {
                        let mut found: Vec<RecallItem> = self
                            .entries
                            .iter()
                            .filter(of_kind)
                            .filter(|e| {
                                e.request == s.request && e.result == ActionResult::Succeeded
                            })
                            .map(item)
                            .collect();
                        found.reverse();
                        found
                    }
                    None => Vec::new(),
                };
                // …then attempts after it that did not succeed, so a failed
                // open is reported as failed, never as opened.
                let after = success.map_or(0, |s| s.seq);
                let mut failed: Vec<RecallItem> = self
                    .entries
                    .iter()
                    .filter(of_kind)
                    .filter(|e| {
                        e.seq > after
                            && matches!(e.result, ActionResult::Failed | ActionResult::Unresolved)
                    })
                    .map(item)
                    .collect();
                failed.reverse();
                items.extend(failed);
                items
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// What a recall question asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum RecallKind {
    /// "What did you just open?"
    Opened,
    /// "What did you close?"
    Closed,
    /// "What did you just do?"
    Did,
}

/// One remembered action, from the ledger (trusted data only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RecallItem {
    pub action: StepAction,
    pub application: Option<ApplicationSummary>,
    pub result: ActionResult,
}

/// How a request was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Modality {
    Typed,
    Voice,
}

/// Where a reference ("close it", "both", "what did you open?") was
/// resolved from (diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ReferenceSource {
    /// The session's recent applications.
    Entities,
    /// The action ledger.
    Ledger,
    /// The open question's trusted candidates.
    Clarification,
}

/// Session diagnostics for Developer Mode (transient; no words, no prompts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionTrace {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub session: u64,
    pub modality: Modality,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub active_entities: usize,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub ledger_entries: usize,
    /// The last successful action after this request.
    pub last_action: Option<RecallItem>,
    pub reference: Option<ReferenceSource>,
    /// Why this request's action ran, when one ran.
    pub authorization: Option<Authorization>,
}

/// The conversation: entities and the ledger, under one id.
#[derive(Debug, Clone)]
pub struct ConversationSession {
    pub id: u64,
    pub entities: SessionContext,
    pub ledger: ActionLedger,
    /// The current request (turn) number.
    pub request: u64,
}

impl Default for ConversationSession {
    fn default() -> Self {
        Self {
            id: 1,
            entities: SessionContext::default(),
            ledger: ActionLedger::default(),
            request: 0,
        }
    }
}

impl ConversationSession {
    /// "New conversation": a fresh id; nothing from before can be referred
    /// to. Preferences, permissions and models are not part of it.
    pub fn reset(&mut self) {
        self.id += 1;
        self.entities.reset();
        self.ledger.clear();
        self.request = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::model::AppSource;

    fn app(name: &str) -> ApplicationSummary {
        ApplicationSummary {
            id: name.to_lowercase(),
            display_name: name.to_owned(),
            source: AppSource::StartMenu,
        }
    }

    fn record(
        ledger: &mut ActionLedger,
        request: u64,
        action: StepAction,
        name: &str,
        result: ActionResult,
    ) {
        let tool = match action {
            StepAction::Close => "system.close_application",
            _ => "system.open_application",
        };
        ledger.record(
            request,
            request,
            ToolId::new(tool).unwrap(),
            action,
            Some(app(name)),
            result,
            CallOrigin::User,
            None,
            Some(Authorization::Policy),
        );
    }

    fn names(items: &[RecallItem]) -> Vec<(&str, ActionResult)> {
        items
            .iter()
            .map(|i| {
                (
                    i.application
                        .as_ref()
                        .map_or("", |a| a.display_name.as_str()),
                    i.result,
                )
            })
            .collect()
    }

    #[test]
    fn what_was_just_opened_is_the_last_successful_open() {
        let mut ledger = ActionLedger::default();
        record(
            &mut ledger,
            1,
            StepAction::Open,
            "Chrome",
            ActionResult::Succeeded,
        );
        record(
            &mut ledger,
            2,
            StepAction::Open,
            "Excel",
            ActionResult::Succeeded,
        );
        assert_eq!(
            names(&ledger.recall(RecallKind::Opened)),
            vec![("Excel", ActionResult::Succeeded)]
        );
    }

    #[test]
    fn a_failed_open_is_never_remembered_as_opened() {
        let mut ledger = ActionLedger::default();
        record(
            &mut ledger,
            1,
            StepAction::Open,
            "Chrome",
            ActionResult::Succeeded,
        );
        record(
            &mut ledger,
            2,
            StepAction::Open,
            "Excel",
            ActionResult::Failed,
        );
        assert_eq!(
            names(&ledger.recall(RecallKind::Opened)),
            vec![
                ("Chrome", ActionResult::Succeeded),
                ("Excel", ActionResult::Failed)
            ]
        );
        let mut only_failed = ActionLedger::default();
        record(
            &mut only_failed,
            1,
            StepAction::Open,
            "Excel",
            ActionResult::Unresolved,
        );
        assert!(
            only_failed
                .recall(RecallKind::Opened)
                .iter()
                .all(|i| i.result != ActionResult::Succeeded)
        );
    }

    #[test]
    fn a_plan_that_opened_two_applications_recalls_both_in_order() {
        let mut ledger = ActionLedger::default();
        record(
            &mut ledger,
            3,
            StepAction::Open,
            "Chrome",
            ActionResult::Succeeded,
        );
        record(
            &mut ledger,
            3,
            StepAction::Open,
            "Outlook",
            ActionResult::Succeeded,
        );
        assert_eq!(
            names(&ledger.recall(RecallKind::Opened)),
            vec![
                ("Chrome", ActionResult::Succeeded),
                ("Outlook", ActionResult::Succeeded)
            ]
        );
    }

    #[test]
    fn what_was_done_is_the_latest_request_with_real_outcomes() {
        let mut ledger = ActionLedger::default();
        record(
            &mut ledger,
            1,
            StepAction::Open,
            "Word",
            ActionResult::Succeeded,
        );
        record(
            &mut ledger,
            2,
            StepAction::Open,
            "Chrome",
            ActionResult::Succeeded,
        );
        record(
            &mut ledger,
            2,
            StepAction::Open,
            "Outlook",
            ActionResult::Failed,
        );
        assert_eq!(
            names(&ledger.recall(RecallKind::Did)),
            vec![
                ("Chrome", ActionResult::Succeeded),
                ("Outlook", ActionResult::Failed)
            ]
        );
    }

    #[test]
    fn nothing_done_recalls_nothing() {
        let ledger = ActionLedger::default();
        for kind in [RecallKind::Opened, RecallKind::Closed, RecallKind::Did] {
            assert!(ledger.recall(kind).is_empty());
        }
    }

    #[test]
    fn the_ledger_is_bounded_and_reset_with_the_conversation() {
        let mut session = ConversationSession::default();
        for i in 0..(MAX_LEDGER_ENTRIES as u64 + 10) {
            record(
                &mut session.ledger,
                i,
                StepAction::Open,
                "Excel",
                ActionResult::Succeeded,
            );
        }
        assert_eq!(session.ledger.len(), MAX_LEDGER_ENTRIES);
        let id = session.id;
        session.reset();
        assert!(session.ledger.is_empty());
        assert_ne!(session.id, id);
        assert!(session.ledger.recall(RecallKind::Opened).is_empty());
    }
}
