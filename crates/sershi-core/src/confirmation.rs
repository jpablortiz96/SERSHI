//! Trusted confirmations.
//!
//! When policy requires approval, the exact tool call is stored here, in the
//! core, under a random one-time id. The UI receives only a description built
//! from structured fields (action, trusted subject, risk) and can answer only
//! with a decision for that id. Rust decides whether the id is known, pending,
//! unexpired and for the same tool; the stored call — never anything sent by
//! the UI — is what runs.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::apps::ApplicationSummary;
use crate::ids::ToolId;
use crate::policy::ConfirmationReason;
use crate::tool::{RiskLevel, ToolCall};

/// How long a confirmation stays valid.
pub const CONFIRMATION_TTL_MS: u64 = 90_000;

/// Upper bound on simultaneously pending confirmations.
const MAX_PENDING: usize = 8;

/// What the user is asked to approve. The UI renders localized copy per
/// action; it never displays text supplied by a tool or a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationAction {
    CloseApplication,
    /// Generic fallback for tools without a dedicated confirmation.
    RunTool,
}

/// The resolved target of a confirmation, from trusted discovery data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ConfirmationSubject {
    Application { application: ApplicationSummary },
}

/// A random, one-time identifier (32 lowercase hex characters).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, type = "string"))]
pub struct ConfirmationId(String);

impl Serialize for ConfirmationId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ConfirmationId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.len() == 32 && raw.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
            Ok(Self(raw))
        } else {
            Err(serde::de::Error::custom("invalid confirmation id"))
        }
    }
}

impl ConfirmationId {
    fn random() -> Self {
        // RandomState is seeded from the OS; two hashers give 128 bits.
        let mut out = String::with_capacity(32);
        for salt in [0x05e2_54a1_u64, 0x00c0_f1e5_u64] {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(salt);
            out.push_str(&format!("{:016x}", h.finish()));
        }
        Self(out)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What the UI shows. Contains no free text from tools, models or input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationRequest {
    pub id: ConfirmationId,
    pub tool_id: ToolId,
    pub action: ConfirmationAction,
    pub subject: Option<ConfirmationSubject>,
    pub risk: RiskLevel,
    pub reason: ConfirmationReason,
    /// Whether "remember this decision" may be offered. Always false until
    /// persistent grants exist (v0.1 settings store); never true for high risk.
    pub can_remember: bool,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expires_at_ms: u64,
}

/// Everything needed to create a confirmation, produced by the executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationDraft {
    pub call: ToolCall,
    pub action: ConfirmationAction,
    pub subject: Option<ConfirmationSubject>,
    pub risk: RiskLevel,
    pub reason: ConfirmationReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingConfirmation {
    pub request: ConfirmationRequest,
    /// The exact call that runs on approval.
    pub call: ToolCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConfirmationError {
    #[error("unknown or already decided confirmation")]
    Unknown,
    #[error("confirmation expired")]
    Expired,
    #[error("confirmation belongs to a different tool")]
    ToolMismatch,
}

#[derive(Debug, Default, Clone)]
pub struct ConfirmationStore {
    pending: Vec<PendingConfirmation>,
}

impl ConfirmationStore {
    pub fn create(&mut self, draft: ConfirmationDraft, now_ms: u64) -> ConfirmationRequest {
        if self.pending.len() >= MAX_PENDING {
            self.pending.remove(0);
        }
        let request = ConfirmationRequest {
            id: ConfirmationId::random(),
            tool_id: draft.call.tool_id.clone(),
            action: draft.action,
            subject: draft.subject,
            risk: draft.risk,
            reason: draft.reason,
            can_remember: false,
            expires_at_ms: now_ms.saturating_add(CONFIRMATION_TTL_MS),
        };
        self.pending.push(PendingConfirmation {
            request: request.clone(),
            call: draft.call,
        });
        request
    }

    /// Removes and returns a pending confirmation. One-time: whatever the
    /// result, the id cannot be used again.
    pub fn take(
        &mut self,
        id: &ConfirmationId,
        tool_id: &ToolId,
        now_ms: u64,
    ) -> Result<PendingConfirmation, ConfirmationError> {
        let index = self
            .pending
            .iter()
            .position(|p| &p.request.id == id)
            .ok_or(ConfirmationError::Unknown)?;
        let pending = self.pending.remove(index);
        if &pending.request.tool_id != tool_id {
            return Err(ConfirmationError::ToolMismatch);
        }
        if now_ms >= pending.request.expires_at_ms {
            return Err(ConfirmationError::Expired);
        }
        Ok(pending)
    }

    /// Removes and returns every expired confirmation.
    pub fn expire(&mut self, now_ms: u64) -> Vec<PendingConfirmation> {
        let (expired, live) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| now_ms >= p.request.expires_at_ms);
        self.pending = live;
        expired
    }

    /// Removes every pending confirmation (e.g. a new request replaced them).
    pub fn cancel_all(&mut self) -> Vec<PendingConfirmation> {
        std::mem::take(&mut self.pending)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn current(&self) -> Option<&ConfirmationRequest> {
        self.pending.last().map(|p| &p.request)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::tool::CallOrigin;

    fn draft(tool: &str) -> ConfirmationDraft {
        ConfirmationDraft {
            call: ToolCall::new(
                ToolId::new(tool).unwrap(),
                json!({"application": "Spotify"}),
                CallOrigin::User,
            ),
            action: ConfirmationAction::CloseApplication,
            subject: None,
            risk: RiskLevel::Sensitive,
            reason: ConfirmationReason::PermissionUndecided,
        }
    }

    fn tool(id: &str) -> ToolId {
        ToolId::new(id).unwrap()
    }

    #[test]
    fn ids_are_random_and_unguessable_length() {
        let a = ConfirmationId::random();
        let b = ConfirmationId::random();
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 32);
    }

    #[test]
    fn a_confirmation_can_be_used_exactly_once() {
        let mut store = ConfirmationStore::default();
        let request = store.create(draft("system.close_application"), 0);
        let taken = store.take(&request.id, &tool("system.close_application"), 1);
        assert_eq!(
            taken.map(|p| p.call.input),
            Ok(json!({"application": "Spotify"}))
        );
        assert_eq!(
            store.take(&request.id, &tool("system.close_application"), 2),
            Err(ConfirmationError::Unknown)
        );
    }

    #[test]
    fn expired_confirmations_never_run_and_are_consumed() {
        let mut store = ConfirmationStore::default();
        let request = store.create(draft("system.close_application"), 1_000);
        assert_eq!(request.expires_at_ms, 1_000 + CONFIRMATION_TTL_MS);
        let late = request.expires_at_ms;
        assert_eq!(
            store.take(&request.id, &tool("system.close_application"), late),
            Err(ConfirmationError::Expired)
        );
        assert_eq!(
            store.take(&request.id, &tool("system.close_application"), 0),
            Err(ConfirmationError::Unknown)
        );
    }

    #[test]
    fn a_decision_must_name_the_pending_tool() {
        let mut store = ConfirmationStore::default();
        let request = store.create(draft("system.close_application"), 0);
        assert_eq!(
            store.take(&request.id, &tool("files.delete"), 1),
            Err(ConfirmationError::ToolMismatch)
        );
        // The mismatch consumed it: it cannot be retried with the right tool.
        assert_eq!(
            store.take(&request.id, &tool("system.close_application"), 1),
            Err(ConfirmationError::Unknown)
        );
    }

    #[test]
    fn malformed_ids_are_rejected_at_the_boundary() {
        for bad in [
            "\"\"",
            "\"abc\"",
            "\"<script>\"",
            "42",
            &format!("\"{}\"", "G".repeat(32)),
        ] {
            assert!(
                serde_json::from_str::<ConfirmationId>(bad).is_err(),
                "{bad}"
            );
        }
        let good = format!("\"{}\"", "a1".repeat(16));
        assert!(serde_json::from_str::<ConfirmationId>(&good).is_ok());
    }

    #[test]
    fn forged_ids_are_unknown() {
        let mut store = ConfirmationStore::default();
        store.create(draft("system.close_application"), 0);
        let forged = ConfirmationId("0".repeat(32));
        assert_eq!(
            store.take(&forged, &tool("system.close_application"), 1),
            Err(ConfirmationError::Unknown)
        );
    }

    #[test]
    fn expiry_sweeps_only_expired_requests() {
        let mut store = ConfirmationStore::default();
        store.create(draft("system.close_application"), 0);
        let fresh = store.create(draft("system.close_application"), 50_000);
        let expired = store.expire(CONFIRMATION_TTL_MS);
        assert_eq!(expired.len(), 1);
        assert_eq!(store.current().map(|r| &r.id), Some(&fresh.id));
    }

    #[test]
    fn remembering_is_never_offered_yet() {
        let mut store = ConfirmationStore::default();
        assert!(
            !store
                .create(draft("system.close_application"), 0)
                .can_remember
        );
    }
}
