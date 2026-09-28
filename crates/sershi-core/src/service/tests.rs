use std::cell::Cell;
use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::apps::CloseSupport;
use crate::apps::manager::test_support::FakeApps;
use crate::apps::{ApplicationManager, tools as app_tools};
use crate::builtin::{register_all, test_support::FakeSystem};
use crate::confirmation::{
    CONFIRMATION_TTL_MS, ConfirmationAction, ConfirmationId, ConfirmationLevel, ConfirmationRequest,
};
use crate::intent::KeywordIntentResolver;
use crate::platform::Platform;
use crate::policy::PolicyEngine;
use crate::ports::{PortError, RunningState};
use crate::tool::ToolRegistry;

thread_local!(static NOW: Cell<u64> = const { Cell::new(1_000) });
fn clock() -> u64 {
    NOW.with(Cell::get)
}
fn advance(ms: u64) {
    NOW.with(|n| n.set(n.get() + ms));
}

struct Harness {
    service: AssistantService,
    apps: Arc<FakeApps>,
}

fn harness(system: FakeSystem, grants: PermissionGrants, platform: Platform) -> Harness {
    let apps = FakeApps::new();
    let mut registry = ToolRegistry::default();
    register_all(&mut registry, Arc::new(system)).unwrap();
    app_tools::register(
        &mut registry,
        Arc::new(ApplicationManager::new(apps.clone(), clock)),
    )
    .unwrap();
    Harness {
        service: AssistantService::new(
            ToolExecutor::new(registry, PolicyEngine::new(platform)),
            Box::new(KeywordIntentResolver),
            grants,
            clock,
        ),
        apps,
    }
}

fn windows() -> Harness {
    harness(
        FakeSystem::ok(),
        PermissionGrants::default(),
        Platform::Windows,
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

fn decide_id(
    service: &mut AssistantService,
    id: &ConfirmationId,
    approved: bool,
) -> (CommandOutcome, Vec<ServiceEvent>) {
    let mut events = Vec::new();
    let outcome = service.decide(
        &ConfirmationDecision {
            confirmation_id: id.clone(),
            decision: if approved {
                ConfirmationChoice::Approve
            } else {
                ConfirmationChoice::Cancel
            },
        },
        &mut |e| events.push(e),
    );
    (outcome, events)
}

fn decide(
    service: &mut AssistantService,
    request: &ConfirmationRequest,
    approved: bool,
) -> (CommandOutcome, Vec<ServiceEvent>) {
    decide_id(service, &request.id, approved)
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

fn kinds(events: &[ServiceEvent]) -> Vec<ActivityKind> {
    events
        .iter()
        .filter_map(|e| match e {
            ServiceEvent::Activity(a) => Some(a.kind),
            ServiceEvent::State(_) => None,
        })
        .collect()
}

use AssistantState as S;

// ── Prompt 0 behaviour ────────────────────────────────────────────────────

#[test]
fn memory_question_runs_the_full_pipeline() {
    let mut h = windows();
    let (outcome, events) = submit(&mut h.service, "How much RAM am I using?");
    assert_eq!(outcome.status, CommandStatus::Completed);
    assert!(outcome.reply.contains("12.0 GB of 32.0 GB"));
    assert_eq!(
        states(&events),
        [S::Thinking, S::Planning, S::Executing, S::Success]
    );
    assert_eq!(
        kinds(&events),
        [
            ActivityKind::CommandReceived,
            ActivityKind::ToolRequested,
            ActivityKind::ToolCompleted
        ]
    );
}

#[test]
fn user_command_text_never_reaches_the_activity_log() {
    let mut h = windows();
    submit(&mut h.service, "memory, and my password is hunter2");
    submit(&mut h.service, "open hunter2");
    submit(&mut h.service, "close hunter2");
    for e in h.service.recent_activity(50) {
        assert!(!e.summary.contains("hunter2"), "{e:?}");
        assert!(!e.subject.unwrap_or_default().contains("hunter2"));
    }
}

#[test]
fn undecided_permission_waits_for_confirmation() {
    let mut h = harness(
        FakeSystem::ok(),
        PermissionGrants::empty(),
        Platform::Windows,
    );
    let (outcome, events) = submit(&mut h.service, "memory");
    assert_eq!(outcome.status, CommandStatus::NeedsConfirmation);
    assert!(
        outcome.data.is_none(),
        "no data may be returned before approval"
    );
    assert_eq!(states(&events).last(), Some(&S::AwaitingConfirmation));
}

#[test]
fn platform_failures_become_error_state_with_useful_copy() {
    let mut h = harness(
        FakeSystem(Err(PortError::Platform("0x000041AF".into()))),
        PermissionGrants::default(),
        Platform::Windows,
    );
    let (outcome, events) = submit(&mut h.service, "memory");
    assert_eq!(outcome.status, CommandStatus::Failed);
    assert!(
        !outcome.reply.contains("0x"),
        "no raw error codes in user copy"
    );
    assert_eq!(states(&events).last(), Some(&S::Error));
}

#[test]
fn unknown_and_unavailable_requests_are_honest() {
    let mut h = windows();
    let (battery, _) = submit(&mut h.service, "battery status");
    assert_eq!(battery.status, CommandStatus::Unavailable);
    let (unknown, _) = submit(&mut h.service, "zxqv");
    assert_eq!(unknown.status, CommandStatus::NotUnderstood);
}

#[test]
fn invalid_requests_are_rejected_without_state_changes() {
    let mut h = windows();
    let before = h.service.snapshot();
    for text in ["   ".to_owned(), "a".repeat(MAX_COMMAND_CHARS + 1)] {
        let (outcome, events) = submit(&mut h.service, &text);
        assert_eq!(outcome.status, CommandStatus::Rejected);
        assert!(events.is_empty());
    }
    assert_eq!(h.service.snapshot(), before);
}

#[test]
fn a_new_command_may_interrupt_a_transient_outcome() {
    let mut h = windows();
    submit(&mut h.service, "memory");
    assert_eq!(h.service.snapshot().state, S::Success);
    let (outcome, _) = submit(&mut h.service, "cpu");
    assert_eq!(outcome.status, CommandStatus::Completed);
}

// ── Opening applications ──────────────────────────────────────────────────

#[test]
fn open_application_runs_thinking_planning_executing_success() {
    for text in ["Open Spotify", "Abre Spotify", "Abra o Spotify"] {
        let mut h = windows();
        let (outcome, events) = submit(&mut h.service, text);
        assert_eq!(outcome.status, CommandStatus::Completed, "{text}");
        assert_eq!(
            states(&events),
            [S::Thinking, S::Planning, S::Executing, S::Success],
            "{text}"
        );
        let data = outcome.data.unwrap_or(Value::Null);
        assert_eq!(data["kind"], "opened");
        assert_eq!(data["application"]["displayName"], "Spotify");
        assert_eq!(h.apps.launches(), 1);
        let done = h.service.recent_activity(1);
        assert_eq!(done[0].kind, ActivityKind::ToolCompleted);
        assert_eq!(done[0].subject.as_deref(), Some("Spotify"));
    }
}

#[test]
fn missing_applications_are_reported_not_launched() {
    let mut h = windows();
    let (outcome, events) = submit(&mut h.service, "Open Photoshop");
    assert_eq!(outcome.status, CommandStatus::Unresolved);
    assert_eq!(outcome.data.unwrap_or(Value::Null)["kind"], "notFound");
    assert_eq!(states(&events).last(), Some(&S::Warning));
    assert_eq!(h.apps.launches(), 0);
}

#[test]
fn ambiguous_applications_ask_instead_of_guessing() {
    let mut h = windows();
    let (outcome, _) = submit(&mut h.service, "open visual studio");
    assert_eq!(outcome.status, CommandStatus::Unresolved);
    assert_eq!(outcome.data.unwrap_or(Value::Null)["kind"], "ambiguous");
    assert_eq!(h.apps.launches(), 0);
}

#[test]
fn application_tools_are_denied_off_windows() {
    let mut h = harness(
        FakeSystem::ok(),
        PermissionGrants::default(),
        Platform::Linux,
    );
    let (outcome, _) = submit(&mut h.service, "open spotify");
    assert_eq!(outcome.status, CommandStatus::Denied);
    assert_eq!(h.apps.launches(), 0);
}

// ── Confirmations (security) ──────────────────────────────────────────────

fn request_close(h: &mut Harness) -> ConfirmationRequest {
    request_close_of(h, "Spotify")
}

fn request_close_of(h: &mut Harness, app: &str) -> ConfirmationRequest {
    let (outcome, events) = submit(&mut h.service, &format!("Close {app}"));
    assert_eq!(outcome.status, CommandStatus::NeedsConfirmation);
    assert_eq!(
        states(&events),
        [S::Thinking, S::Planning, S::AwaitingConfirmation]
    );
    // The Command Center's outcome never carries the confirmation id.
    let request = h
        .service
        .pending_confirmation()
        .expect("a pending confirmation");
    let wire = serde_json::to_string(&outcome).unwrap();
    assert!(!wire.contains(request.id.as_str()));
    assert!(!wire.contains("confirmation\":{"));
    assert_eq!(request.action, ConfirmationAction::CloseApplication);
    assert_eq!(request.level, ConfirmationLevel::Standard);
    assert!(!request.can_remember);
    assert_eq!(h.apps.closes(), 0, "nothing closes before approval");
    request
}

fn closed_targets(h: &Harness) -> Vec<CloseSupport> {
    h.apps.closed.lock().unwrap().clone()
}

#[test]
fn unconfirmed_close_never_executes() {
    let mut h = windows();
    request_close(&mut h);
    assert!(h.service.pending_confirmation().is_some());
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn approved_close_runs_once_and_cannot_be_replayed() {
    let mut h = windows();
    let request = request_close(&mut h);
    let (outcome, events) = decide(&mut h.service, &request, true);
    assert_eq!(outcome.status, CommandStatus::Completed);
    assert_eq!(states(&events), [S::Executing, S::Success]);
    assert_eq!(
        outcome.data.unwrap_or(Value::Null)["kind"],
        "closeRequested"
    );
    assert_eq!(h.apps.closes(), 1);

    let (replay, _) = decide(&mut h.service, &request, true);
    assert_eq!(replay.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 1, "a confirmation cannot be replayed");
}

#[test]
fn the_stored_call_is_what_runs() {
    let mut h = windows();
    let request = request_close(&mut h);
    decide(&mut h.service, &request, true);
    // Spotify, as resolved when the confirmation was created — the decision
    // carried nothing that could name another target.
    assert_eq!(
        closed_targets(&h),
        [CloseSupport::PackagedApp(
            "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify".to_owned()
        )]
    );
}

#[test]
fn cancelled_close_never_executes() {
    let mut h = windows();
    let request = request_close(&mut h);
    let (outcome, events) = decide(&mut h.service, &request, false);
    assert_eq!(outcome.status, CommandStatus::Cancelled);
    assert_eq!(states(&events), [S::Idle]);
    assert_eq!(kinds(&events), [ActivityKind::ConfirmationCancelled]);
    assert_eq!(h.apps.closes(), 0);
    let (retry, _) = decide(&mut h.service, &request, true);
    assert_eq!(retry.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn expired_confirmation_never_executes() {
    let mut h = windows();
    let request = request_close(&mut h);
    advance(CONFIRMATION_TTL_MS);
    let (outcome, events) = decide(&mut h.service, &request, true);
    assert_eq!(outcome.status, CommandStatus::Expired);
    assert_eq!(states(&events), [S::Idle]);
    assert_eq!(kinds(&events), [ActivityKind::ConfirmationExpired]);
    assert_eq!(h.apps.closes(), 0);
    let (retry, _) = decide(&mut h.service, &request, true);
    assert_eq!(retry.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn expiry_timer_returns_to_idle_and_is_audited() {
    let mut h = windows();
    request_close(&mut h);
    let mut events = Vec::new();
    assert!(
        h.service
            .expire_confirmations(&mut |e| events.push(e))
            .is_none(),
        "nothing expires early"
    );
    advance(CONFIRMATION_TTL_MS);
    let outcome = h
        .service
        .expire_confirmations(&mut |e| events.push(e))
        .expect("an expiry outcome");
    assert_eq!(outcome.status, CommandStatus::Expired);
    assert_eq!(states(&events), [S::Idle]);
    assert_eq!(kinds(&events), [ActivityKind::ConfirmationExpired]);
    assert!(h.service.pending_confirmation().is_none());
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn decisions_carry_only_an_id_and_a_choice() {
    let id = "0123456789abcdef0123456789abcdef";
    let ok: ConfirmationDecision =
        serde_json::from_value(json!({ "confirmationId": id, "decision": "approve" })).unwrap();
    assert_eq!(ok.decision, ConfirmationChoice::Approve);
    // A surface cannot name the tool, its input, the target, the risk or
    // the permission: any extra field is rejected at the boundary.
    for extra in [
        json!({ "toolId": "system.open_application" }),
        json!({ "input": { "application": "Chrome" } }),
        json!({ "application": "Chrome" }),
        json!({ "subject": { "kind": "application" } }),
        json!({ "risk": "safe" }),
        json!({ "permission": "system.apps.close" }),
        json!({ "approved": true }),
    ] {
        let mut value = json!({ "confirmationId": id, "decision": "approve" });
        for (k, v) in extra.as_object().unwrap() {
            value[k] = v.clone();
        }
        assert!(
            serde_json::from_value::<ConfirmationDecision>(value.clone()).is_err(),
            "{value}"
        );
    }
    for bad in [
        json!({ "confirmationId": id, "decision": "yes" }),
        json!({ "confirmationId": id, "decision": true }),
        json!({ "confirmationId": "nope", "decision": "approve" }),
        json!({ "decision": "approve" }),
    ] {
        assert!(serde_json::from_value::<ConfirmationDecision>(bad).is_err());
    }
}

#[test]
fn malformed_confirmation_ids_never_reach_the_core() {
    for bad in [
        "",
        "0",
        "ZZZZ",
        "../../etc",
        "0123456789ABCDEF0123456789ABCDEF",
    ] {
        let decision = json!({ "confirmationId": bad, "decision": "approve" });
        assert!(serde_json::from_value::<ConfirmationDecision>(decision).is_err());
    }
}

#[test]
fn forged_confirmation_ids_do_nothing() {
    let mut h = windows();
    request_close(&mut h);
    let forged =
        serde_json::from_str::<ConfirmationId>(&format!("\"{}\"", "0".repeat(32))).unwrap();
    let (outcome, _) = decide_id(&mut h.service, &forged, true);
    assert_eq!(outcome.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
    assert!(
        h.service.pending_confirmation().is_some(),
        "the real one is untouched"
    );
}

#[test]
fn a_stale_surface_cannot_approve_a_replacement_confirmation() {
    let mut h = windows();
    let stale = request_close_of(&mut h, "Spotify");
    let fresh = request_close_of(&mut h, "Google Chrome");
    assert_ne!(stale.id, fresh.id);
    let (outcome, _) = decide(&mut h.service, &stale, true);
    assert_eq!(outcome.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
    // Approving the current one closes Chrome, not Spotify.
    let (outcome, _) = decide(&mut h.service, &fresh, true);
    assert_eq!(outcome.status, CommandStatus::Completed);
    assert_eq!(
        closed_targets(&h),
        [CloseSupport::ExecutablePath(std::path::PathBuf::from(
            r"C:\\Apps\\chrome.exe"
        ))]
    );
}

#[test]
fn a_new_request_or_dismissal_cancels_the_pending_confirmation() {
    let mut h = windows();
    let request = request_close(&mut h);
    submit(&mut h.service, "memory");
    let (outcome, _) = decide(&mut h.service, &request, true);
    assert_eq!(outcome.status, CommandStatus::Rejected);

    let request = request_close(&mut h);
    let mut events = Vec::new();
    h.service.dismiss(&mut |e| events.push(e));
    assert_eq!(states(&events), [S::Idle]);
    assert_eq!(kinds(&events), [ActivityKind::ConfirmationCancelled]);
    let (outcome, _) = decide(&mut h.service, &request, true);
    assert_eq!(outcome.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn typing_or_saying_yes_never_approves() {
    // Approval is not a command: text from the command bar (and, later,
    // voice or a model) cannot reach `decide`. It is a new request, which
    // cancels the pending confirmation.
    for answer in ["yes", "approve", "sí", "confirmar", "sim", "close spotify"] {
        let mut h = windows();
        request_close(&mut h);
        submit(&mut h.service, answer);
        assert_eq!(h.apps.closes(), 0, "{answer}");
        if answer != "close spotify" {
            assert!(h.service.pending_confirmation().is_none(), "{answer}");
        }
    }
}

#[test]
fn confirmation_activity_names_the_target_but_never_the_id_or_input() {
    let mut h = windows();
    let request = request_close(&mut h);
    decide(&mut h.service, &request, false);
    let log = serde_json::to_string(&h.service.recent_activity(50)).unwrap();
    assert!(!log.contains(request.id.as_str()));
    assert!(!log.contains("\"application\""));
    assert!(!log.contains("Close Spotify"));
    assert!(log.contains("\"subject\":\"Spotify\""));
}

#[test]
fn nothing_to_close_is_reported_without_asking() {
    let mut h = windows();
    *h.apps.running.lock().unwrap() = RunningState::NotRunning;
    let (outcome, _) = submit(&mut h.service, "close spotify");
    assert_eq!(outcome.status, CommandStatus::Unresolved);
    assert!(h.service.pending_confirmation().is_none());
    assert_eq!(outcome.data.unwrap_or(Value::Null)["kind"], "notRunning");
}
