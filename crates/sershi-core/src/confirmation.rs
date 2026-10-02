//! Trusted confirmations.
//!
//! When policy requires approval, the exact tool call is stored here, in the
//! core, under a one-time id drawn from the operating system's CSPRNG. The
//! trusted confirmation surface receives only a description built from
//! structured fields (action, trusted subject, risk) and can answer only with
//! a decision for that id. The stored call — never anything sent by a
//! surface — is what runs.
//!
//! One confirmation is pending at a time: SERSHI has one user, and a second
//! concurrent approval adds no value while making the user's decision harder
//! to read. A new request cancels the pending one (nothing runs).
//!
//! State lives only in memory. A restart or crash discards it, and nothing
//! here is ever persisted (no storage, logs or files).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::apps::ApplicationSummary;
use crate::ids::ToolId;
use crate::permission::{ConfigurablePermission, PermissionSetting};
use crate::policy::ConfirmationReason;
use crate::tool::{RiskLevel, ToolCall};

/// How long a confirmation stays valid. Long enough to read and decide
/// deliberately; short enough that no stale authorization lingers.
pub const CONFIRMATION_TTL_MS: u64 = 60_000;

/// Random bytes in a confirmation id (128 bits).
pub const CONFIRMATION_ID_BYTES: usize = 16;

/// What the user is asked to approve. The UI renders localized copy per
/// action; it never displays text supplied by a tool or a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationAction {
    CloseApplication,
    /// Close several applications at once (Gate 4.1.1): one decision for
    /// exactly the applications listed, nothing else.
    CloseApplications,
    /// Settings asked to stop confirming a permission (Gate 4.1).
    ChangePermission,
    /// Generic fallback for tools without a dedicated confirmation.
    RunTool,
}

/// How strong the approval interaction must be.
///
/// Today both levels use the same explicit Cancel / Approve interaction;
/// `highRisk` is presented with stronger warning copy. Reserved for later
/// (not implemented): a `critical` level whose approval requires more than a
/// click — hold to confirm, typing the target's name, a second confirmation,
/// or Windows Hello for payments, credentials, security settings and
/// permanent deletion. See docs/SECURITY.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationLevel {
    Standard,
    HighRisk,
}

impl ConfirmationLevel {
    pub fn for_risk(risk: RiskLevel) -> Self {
        match risk {
            RiskLevel::HighRisk => Self::HighRisk,
            RiskLevel::Safe | RiskLevel::Sensitive => Self::Standard,
        }
    }
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
    Application {
        application: ApplicationSummary,
    },
    /// A permission and the setting it would get.
    Permission {
        permission: ConfigurablePermission,
        setting: PermissionSetting,
    },
    /// The exact applications a grouped close covers (trusted catalog
    /// entries, in order).
    Applications {
        applications: Vec<ApplicationSummary>,
    },
    /// One application's close setting (Gate 4.1.1).
    ApplicationPermission {
        app_id: String,
        display_name: String,
        setting: PermissionSetting,
    },
}

/// Most applications one grouped close may cover.
pub const MAX_BATCH: usize = 5;

/// One call of a grouped close, with the target it was resolved to when
/// the confirmation was created (re-verified on approval).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchItem {
    pub call: ToolCall,
    pub subject: Option<ConfirmationSubject>,
}

/// Display id of a permission change in the confirmation window.
pub const PERMISSION_CHANGE_TOOL: &str = "settings.permissions";

/// A one-time identifier: 128 bits from the OS CSPRNG, as 32 lowercase hex
/// characters.
///
/// Not a password. Security does not rest on its secrecy: only the core
/// issues ids, only the trusted confirmation surface may present one, and
/// each works once, for one stored action, for a short time. It is
/// unpredictable so that it cannot be guessed or reused across requests.
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
        if is_valid_id(&raw) {
            Ok(Self(raw))
        } else {
            Err(serde::de::Error::custom("invalid confirmation id"))
        }
    }
}

fn is_valid_id(raw: &str) -> bool {
    raw.len() == CONFIRMATION_ID_BYTES * 2
        && raw.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

impl ConfirmationId {
    /// Draws a new id from the operating system's CSPRNG (`getrandom`:
    /// `ProcessPrng` on Windows, `getrandom(2)` on Linux). Fails closed: if
    /// the OS cannot provide randomness, no confirmation is created.
    pub fn generate() -> Result<Self, ConfirmationError> {
        let mut bytes = [0_u8; CONFIRMATION_ID_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| ConfirmationError::RandomnessUnavailable)?;
        Ok(Self(to_hex(&bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What the trusted confirmation surface shows. Contains no free text from
/// tools, models or the user's request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationRequest {
    pub id: ConfirmationId,
    /// For display only (e.g. "Allow Memory usage?"); decisions never carry it.
    pub tool_id: ToolId,
    pub action: ConfirmationAction,
    pub subject: Option<ConfirmationSubject>,
    pub risk: RiskLevel,
    pub level: ConfirmationLevel,
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

/// What runs on approval: stored in the core, never sent by a surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAction {
    /// The exact tool call.
    Tool(ToolCall),
    /// A less restrictive permission setting, requested in Settings.
    Permission {
        permission: ConfigurablePermission,
        setting: PermissionSetting,
    },
    /// A grouped close: exactly these calls, immutable once stored.
    Batch(Vec<BatchItem>),
    /// A less restrictive per-application close setting.
    ApplicationPermission {
        app_id: String,
        display_name: String,
        setting: PermissionSetting,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingConfirmation {
    pub request: ConfirmationRequest,
    pub action: PendingAction,
}

impl PendingConfirmation {
    /// The stored tool call, for a tool confirmation.
    pub fn call(&self) -> Option<&ToolCall> {
        match &self.action {
            PendingAction::Tool(call) => Some(call),
            PendingAction::Permission { .. }
            | PendingAction::Batch(_)
            | PendingAction::ApplicationPermission { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConfirmationError {
    #[error("unknown or already decided confirmation")]
    Unknown,
    #[error("confirmation expired")]
    Expired,
    #[error("the operating system could not provide secure randomness")]
    RandomnessUnavailable,
}

/// The single pending confirmation, if any.
#[derive(Debug, Default, Clone)]
pub struct ConfirmationStore {
    pending: Option<PendingConfirmation>,
}

impl ConfirmationStore {
    /// Stores a draft under a fresh id. Returns the confirmation it replaced,
    /// if any (callers cancel pending confirmations first and audit them).
    pub fn create(
        &mut self,
        draft: ConfirmationDraft,
        now_ms: u64,
    ) -> Result<(ConfirmationRequest, Option<PendingConfirmation>), ConfirmationError> {
        let id = ConfirmationId::generate()?;
        let request = ConfirmationRequest {
            id,
            tool_id: draft.call.tool_id.clone(),
            action: draft.action,
            subject: draft.subject,
            risk: draft.risk,
            level: ConfirmationLevel::for_risk(draft.risk),
            reason: draft.reason,
            can_remember: false,
            expires_at_ms: now_ms.saturating_add(CONFIRMATION_TTL_MS),
        };
        let replaced = self.pending.replace(PendingConfirmation {
            request: request.clone(),
            action: PendingAction::Tool(draft.call),
        });
        Ok((request, replaced))
    }

    /// Stores a grouped close of several applications as one confirmation
    /// (Gate 4.1.1). The stored calls and their resolved targets are the
    /// whole of what an approval can run; nothing can be added later.
    /// `None` without at least two drafts, or with more than [`MAX_BATCH`].
    pub fn create_batch(
        &mut self,
        drafts: Vec<ConfirmationDraft>,
        now_ms: u64,
    ) -> Result<Option<(ConfirmationRequest, Option<PendingConfirmation>)>, ConfirmationError> {
        if drafts.len() < 2 || drafts.len() > MAX_BATCH {
            return Ok(None);
        }
        let id = ConfirmationId::generate()?;
        let applications: Vec<ApplicationSummary> = drafts
            .iter()
            .filter_map(|d| match &d.subject {
                Some(ConfirmationSubject::Application { application }) => Some(application.clone()),
                _ => None,
            })
            .collect();
        let first = &drafts[0];
        let risk = drafts
            .iter()
            .map(|d| d.risk)
            .max_by_key(|r| match r {
                RiskLevel::Safe => 0,
                RiskLevel::Sensitive => 1,
                RiskLevel::HighRisk => 2,
            })
            .unwrap_or(first.risk);
        let request = ConfirmationRequest {
            id,
            tool_id: first.call.tool_id.clone(),
            action: ConfirmationAction::CloseApplications,
            subject: Some(ConfirmationSubject::Applications { applications }),
            risk,
            level: ConfirmationLevel::for_risk(risk),
            reason: first.reason,
            can_remember: false,
            expires_at_ms: now_ms.saturating_add(CONFIRMATION_TTL_MS),
        };
        let items = drafts
            .into_iter()
            .map(|d| BatchItem {
                call: d.call,
                subject: d.subject,
            })
            .collect();
        let replaced = self.pending.replace(PendingConfirmation {
            request: request.clone(),
            action: PendingAction::Batch(items),
        });
        Ok(Some((request, replaced)))
    }

    /// Stores a request to make one application's close setting "Always
    /// allow" (Gate 4.1.1); decided only in the trusted window.
    pub fn create_app_permission(
        &mut self,
        app_id: &str,
        display_name: &str,
        setting: PermissionSetting,
        now_ms: u64,
    ) -> Result<(ConfirmationRequest, Option<PendingConfirmation>), ConfirmationError> {
        let id = ConfirmationId::generate()?;
        let tool_id =
            ToolId::new(PERMISSION_CHANGE_TOOL).map_err(|_| ConfirmationError::Unknown)?;
        let request = ConfirmationRequest {
            id,
            tool_id,
            action: ConfirmationAction::ChangePermission,
            subject: Some(ConfirmationSubject::ApplicationPermission {
                app_id: app_id.to_owned(),
                display_name: display_name.to_owned(),
                setting,
            }),
            risk: RiskLevel::Sensitive,
            level: ConfirmationLevel::Standard,
            reason: ConfirmationReason::PermissionChange,
            can_remember: false,
            expires_at_ms: now_ms.saturating_add(CONFIRMATION_TTL_MS),
        };
        let replaced = self.pending.replace(PendingConfirmation {
            request: request.clone(),
            action: PendingAction::ApplicationPermission {
                app_id: app_id.to_owned(),
                display_name: display_name.to_owned(),
                setting,
            },
        });
        Ok((request, replaced))
    }

    /// Stores a request to make a permission less restrictive (Gate 4.1).
    /// Like any approval, it is decided only in the trusted window.
    pub fn create_permission(
        &mut self,
        permission: ConfigurablePermission,
        setting: PermissionSetting,
        now_ms: u64,
    ) -> Result<(ConfirmationRequest, Option<PendingConfirmation>), ConfirmationError> {
        let id = ConfirmationId::generate()?;
        let tool_id =
            ToolId::new(PERMISSION_CHANGE_TOOL).map_err(|_| ConfirmationError::Unknown)?;
        let request = ConfirmationRequest {
            id,
            tool_id,
            action: ConfirmationAction::ChangePermission,
            subject: Some(ConfirmationSubject::Permission {
                permission,
                setting,
            }),
            risk: RiskLevel::Sensitive,
            level: ConfirmationLevel::Standard,
            reason: ConfirmationReason::PermissionChange,
            can_remember: false,
            expires_at_ms: now_ms.saturating_add(CONFIRMATION_TTL_MS),
        };
        let replaced = self.pending.replace(PendingConfirmation {
            request: request.clone(),
            action: PendingAction::Permission {
                permission,
                setting,
            },
        });
        Ok((request, replaced))
    }

    /// Removes and returns the pending confirmation with this id. One-time:
    /// once taken — approved, cancelled or found expired — the id can never
    /// be used again. An id that doesn't match (a stale surface, a forged
    /// value) is rejected and leaves the current confirmation untouched.
    pub fn take(
        &mut self,
        id: &ConfirmationId,
        now_ms: u64,
    ) -> Result<PendingConfirmation, ConfirmationError> {
        if self.pending.as_ref().map(|p| &p.request.id) != Some(id) {
            return Err(ConfirmationError::Unknown);
        }
        let pending = self.pending.take().ok_or(ConfirmationError::Unknown)?;
        if now_ms >= pending.request.expires_at_ms {
            return Err(ConfirmationError::Expired);
        }
        Ok(pending)
    }

    /// Removes and returns the pending confirmation if it has expired.
    pub fn expire(&mut self, now_ms: u64) -> Option<PendingConfirmation> {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| now_ms >= p.request.expires_at_ms)
        {
            self.pending.take()
        } else {
            None
        }
    }

    /// Removes the pending confirmation (a new request, dismissal or quit).
    pub fn cancel_all(&mut self) -> Option<PendingConfirmation> {
        self.pending.take()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_none()
    }

    pub fn current(&self) -> Option<&ConfirmationRequest> {
        self.pending.as_ref().map(|p| &p.request)
    }
}

/// What the desktop shell must do with the trusted confirmation surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceAction {
    /// Nothing to change.
    Keep,
    /// Create the surface for this confirmation.
    Open(ConfirmationId),
    /// The surface shows a confirmation that no longer exists; show this one
    /// instead (reload, so its input guard re-arms).
    Replace(ConfirmationId),
    /// No confirmation is pending: destroy the surface.
    Close,
}

/// Which confirmation the trusted surface is assigned to. Kept by the shell
/// so that exactly one surface exists per pending confirmation, a surface
/// can only decide the confirmation it was opened for, and closing it
/// cancels that confirmation — never a newer one.
#[derive(Debug, Default, Clone)]
pub struct SurfaceAssignment {
    assigned: Option<ConfirmationId>,
}

impl SurfaceAssignment {
    /// Aligns the surface with the core's pending confirmation.
    pub fn reconcile(&mut self, pending: Option<&ConfirmationId>) -> SurfaceAction {
        match (self.assigned.as_ref(), pending) {
            (None, None) => SurfaceAction::Keep,
            (Some(current), Some(next)) if current == next => SurfaceAction::Keep,
            (None, Some(next)) => {
                self.assigned = Some(next.clone());
                SurfaceAction::Open(next.clone())
            }
            (Some(_), Some(next)) => {
                self.assigned = Some(next.clone());
                SurfaceAction::Replace(next.clone())
            }
            (Some(_), None) => {
                self.assigned = None;
                SurfaceAction::Close
            }
        }
    }

    /// The confirmation the surface currently shows.
    pub fn assigned(&self) -> Option<&ConfirmationId> {
        self.assigned.as_ref()
    }

    /// Whether a decision naming `id` comes from the surface's current
    /// confirmation (a stale surface cannot decide a replacement).
    pub fn authorizes(&self, id: &ConfirmationId) -> bool {
        self.assigned.as_ref() == Some(id)
    }

    /// The user closed the surface: returns the confirmation to cancel.
    pub fn release(&mut self) -> Option<ConfirmationId> {
        self.assigned.take()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::tool::CallOrigin;

    fn draft(application: &str) -> ConfirmationDraft {
        ConfirmationDraft {
            call: ToolCall::new(
                ToolId::new("system.close_application").unwrap(),
                json!({ "application": application }),
                CallOrigin::User,
            ),
            action: ConfirmationAction::CloseApplication,
            subject: None,
            risk: RiskLevel::Sensitive,
            reason: ConfirmationReason::PermissionUndecided,
        }
    }

    fn create(store: &mut ConfirmationStore, application: &str, now: u64) -> ConfirmationRequest {
        store.create(draft(application), now).unwrap().0
    }

    #[test]
    fn confirmation_ids_are_128_bit_random_values() {
        let a = ConfirmationId::generate().unwrap();
        let b = ConfirmationId::generate().unwrap();
        // 16 bytes → 32 lowercase hex characters, accepted by the boundary.
        assert_eq!(a.as_str().len(), 2 * CONFIRMATION_ID_BYTES);
        assert_eq!(CONFIRMATION_ID_BYTES * 8, 128);
        assert!(is_valid_id(a.as_str()));
        let round_trip: ConfirmationId =
            serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
        assert_eq!(round_trip, a);
        // Successive ids differ (no statistical claims; the OS CSPRNG is
        // relied on for unpredictability).
        assert_ne!(a, b);
    }

    #[test]
    fn hex_encoding_is_lowercase_and_complete() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn a_confirmation_can_be_used_exactly_once() {
        let mut store = ConfirmationStore::default();
        let request = create(&mut store, "Spotify", 0);
        let taken = store.take(&request.id, 1);
        assert_eq!(
            taken.map(|p| p.call().map(|c| c.input.clone())),
            Ok(Some(json!({"application": "Spotify"})))
        );
        assert_eq!(store.take(&request.id, 2), Err(ConfirmationError::Unknown));
    }

    #[test]
    fn expired_confirmations_never_run_and_are_consumed() {
        let mut store = ConfirmationStore::default();
        let request = create(&mut store, "Spotify", 1_000);
        assert_eq!(request.expires_at_ms, 1_000 + CONFIRMATION_TTL_MS);
        let late = request.expires_at_ms;
        assert_eq!(
            store.take(&request.id, late),
            Err(ConfirmationError::Expired)
        );
        assert_eq!(store.take(&request.id, 0), Err(ConfirmationError::Unknown));
    }

    #[test]
    fn only_one_confirmation_is_pending_at_a_time() {
        let mut store = ConfirmationStore::default();
        let first = create(&mut store, "Spotify", 0);
        let (second, replaced) = store.create(draft("Notepad"), 1).unwrap();
        assert_eq!(replaced.map(|p| p.request.id), Some(first.id.clone()));
        assert_eq!(store.current().map(|r| &r.id), Some(&second.id));
        // The replaced id is dead.
        assert_eq!(store.take(&first.id, 2), Err(ConfirmationError::Unknown));
    }

    #[test]
    fn a_stale_id_cannot_decide_the_replacement() {
        let mut store = ConfirmationStore::default();
        let stale = create(&mut store, "Spotify", 0);
        store.cancel_all();
        let fresh = create(&mut store, "Notepad", 1);
        assert_eq!(store.take(&stale.id, 2), Err(ConfirmationError::Unknown));
        // The rejected attempt leaves the current confirmation intact.
        assert_eq!(store.current().map(|r| &r.id), Some(&fresh.id));
        assert!(store.take(&fresh.id, 3).is_ok());
    }

    #[test]
    fn malformed_ids_are_rejected_at_the_boundary() {
        for bad in [
            "\"\"",
            "\"abc\"",
            "\"<script>\"",
            "42",
            "null",
            &format!("\"{}\"", "G".repeat(32)),
            &format!("\"{}\"", "A".repeat(32)),
            &format!("\"{}\"", "a".repeat(33)),
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
        create(&mut store, "Spotify", 0);
        let forged = ConfirmationId("0".repeat(32));
        assert_eq!(store.take(&forged, 1), Err(ConfirmationError::Unknown));
        assert!(!store.is_empty());
    }

    #[test]
    fn expiry_removes_only_an_expired_confirmation() {
        let mut store = ConfirmationStore::default();
        let request = create(&mut store, "Spotify", 0);
        assert_eq!(store.expire(CONFIRMATION_TTL_MS - 1), None);
        assert_eq!(
            store.expire(CONFIRMATION_TTL_MS).map(|p| p.request.id),
            Some(request.id)
        );
        assert!(store.is_empty());
    }

    #[test]
    fn remembering_is_never_offered_yet() {
        let mut store = ConfirmationStore::default();
        assert!(!create(&mut store, "Spotify", 0).can_remember);
    }

    #[test]
    fn the_level_follows_the_registered_risk() {
        assert_eq!(
            ConfirmationLevel::for_risk(RiskLevel::Sensitive),
            ConfirmationLevel::Standard
        );
        assert_eq!(
            ConfirmationLevel::for_risk(RiskLevel::HighRisk),
            ConfirmationLevel::HighRisk
        );
    }

    fn id(n: u8) -> ConfirmationId {
        ConfirmationId(to_hex(&[n; CONFIRMATION_ID_BYTES]))
    }

    #[test]
    fn exactly_one_surface_follows_the_pending_confirmation() {
        let mut surface = SurfaceAssignment::default();
        assert_eq!(surface.reconcile(None), SurfaceAction::Keep);
        assert_eq!(surface.reconcile(Some(&id(1))), SurfaceAction::Open(id(1)));
        // Re-reconciling the same confirmation never opens a second surface.
        assert_eq!(surface.reconcile(Some(&id(1))), SurfaceAction::Keep);
        assert_eq!(
            surface.reconcile(Some(&id(2))),
            SurfaceAction::Replace(id(2))
        );
        assert_eq!(surface.reconcile(None), SurfaceAction::Close);
        assert_eq!(surface.assigned(), None);
    }

    #[test]
    fn a_stale_surface_cannot_decide_a_replacement() {
        let mut surface = SurfaceAssignment::default();
        surface.reconcile(Some(&id(1)));
        surface.reconcile(Some(&id(2)));
        assert!(!surface.authorizes(&id(1)));
        assert!(surface.authorizes(&id(2)));
    }

    #[test]
    fn closing_the_surface_releases_only_its_own_confirmation() {
        let mut surface = SurfaceAssignment::default();
        surface.reconcile(Some(&id(1)));
        assert_eq!(surface.release(), Some(id(1)));
        // Already released (e.g. the shell closed it after a decision).
        assert_eq!(surface.release(), None);
        assert!(!surface.authorizes(&id(1)));
    }
}
