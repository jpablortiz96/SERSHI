use std::cell::Cell;
use std::sync::Arc;

use serde_json::Value;

use super::*;
use crate::apps::manager::test_support::FakeApps;
use crate::apps::{ApplicationManager, tools as app_tools};
use crate::builtin::{register_all, test_support::FakeSystem};
use crate::confirmation::{CONFIRMATION_TTL_MS, ConfirmationAction, ConfirmationId};
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

fn decide(
    service: &mut AssistantService,
    request: &crate::confirmation::ConfirmationRequest,
    approved: bool,
) -> (CommandOutcome, Vec<ServiceEvent>) {
    let mut events = Vec::new();
    let outcome = service.decide(
        &ConfirmationDecision {
            confirmation_id: request.id.clone(),
            tool_id: request.tool_id.clone(),
            approved,
        },
        &mut |e| events.push(e),
    );
    (outcome, events)
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

fn request_close(h: &mut Harness) -> crate::confirmation::ConfirmationRequest {
    let (outcome, events) = submit(&mut h.service, "Close Spotify");
    assert_eq!(outcome.status, CommandStatus::NeedsConfirmation);
    assert_eq!(
        states(&events),
        [S::Thinking, S::Planning, S::AwaitingConfirmation]
    );
    let request = outcome.confirmation.expect("a confirmation request");
    assert_eq!(request.action, ConfirmationAction::CloseApplication);
    assert!(!request.can_remember);
    assert_eq!(h.apps.closes(), 0, "nothing closes before approval");
    request
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
fn denied_close_never_executes() {
    let mut h = windows();
    let request = request_close(&mut h);
    let (outcome, events) = decide(&mut h.service, &request, false);
    assert_eq!(outcome.status, CommandStatus::Cancelled);
    assert_eq!(states(&events), [S::Idle]);
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
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn expiry_timer_returns_to_idle_and_is_audited() {
    let mut h = windows();
    request_close(&mut h);
    advance(CONFIRMATION_TTL_MS);
    let mut events = Vec::new();
    assert!(h.service.expire_confirmations(&mut |e| events.push(e)));
    assert_eq!(states(&events), [S::Idle]);
    assert_eq!(kinds(&events), [ActivityKind::ConfirmationExpired]);
    assert!(h.service.pending_confirmation().is_none());
}

#[test]
fn a_decision_must_match_the_pending_tool_call() {
    let mut h = windows();
    let request = request_close(&mut h);
    let mut forged = request.clone();
    forged.tool_id = ToolId::new(app_tools::OPEN_APPLICATION).unwrap();
    let (outcome, _) = decide(&mut h.service, &forged, true);
    assert_eq!(outcome.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
    assert_eq!(h.apps.launches(), 0);
    // The mismatch consumed the confirmation.
    let (retry, _) = decide(&mut h.service, &request, true);
    assert_eq!(retry.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn forged_confirmation_ids_do_nothing() {
    let mut h = windows();
    let request = request_close(&mut h);
    let mut forged = request.clone();
    forged.id = serde_json::from_str::<ConfirmationId>(&format!("\"{}\"", "0".repeat(32))).unwrap();
    let (outcome, _) = decide(&mut h.service, &forged, true);
    assert_eq!(outcome.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
    assert!(
        h.service.pending_confirmation().is_some(),
        "the real one is untouched"
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
    let (outcome, _) = decide(&mut h.service, &request, true);
    assert_eq!(outcome.status, CommandStatus::Rejected);
    assert_eq!(h.apps.closes(), 0);
}

#[test]
fn nothing_to_close_is_reported_without_asking() {
    let mut h = windows();
    *h.apps.running.lock().unwrap() = RunningState::NotRunning;
    let (outcome, _) = submit(&mut h.service, "close spotify");
    assert_eq!(outcome.status, CommandStatus::Unresolved);
    assert!(outcome.confirmation.is_none());
    assert_eq!(outcome.data.unwrap_or(Value::Null)["kind"], "notRunning");
}
