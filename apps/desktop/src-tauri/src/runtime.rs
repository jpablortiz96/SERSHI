//! Process-wide state and event broadcasting.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sershi_core::activity::{ActivityKind, NewActivity};
use sershi_core::apps::{ApplicationManager, CatalogState, tools as app_tools};
use sershi_core::assistant::{AssistantEvent, AssistantSnapshot, AssistantState};
use sershi_core::builtin::{self, BuiltinError};
use sershi_core::confirmation::CONFIRMATION_TTL_MS;
use sershi_core::executor::ToolExecutor;
use sershi_core::intent::KeywordIntentResolver;
use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::permission::PermissionGrants;
use sershi_core::platform::Platform;
use sershi_core::policy::PolicyEngine;
use sershi_core::service::{AssistantService, CommandOutcome, Progress, ServiceEvent};
use sershi_core::tool::ToolRegistry;
use sershi_core::understanding::CLARIFICATION_TTL_MS;
use sershi_platform::SysinfoSystemInfo;
use tauri::{AppHandle, Emitter, Manager};

use crate::confirmation;

/// Event carrying an `AssistantSnapshot` to every window.
pub const STATE_EVENT: &str = "sershi://assistant-state";
/// Event carrying a new `ActivityEntry` to every window.
pub const ACTIVITY_EVENT: &str = "sershi://activity";
/// Event carrying a plan's progress (`PlanReport`) to the Command Center.
pub const PLAN_EVENT: &str = "sershi://plan";

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[derive(Debug)]
pub struct Runtime {
    service: Mutex<AssistantService>,
    pub system: Arc<SysinfoSystemInfo>,
    pub apps: Arc<ApplicationManager>,
}

impl Runtime {
    pub fn new() -> Result<Self, BuiltinError> {
        let system = Arc::new(SysinfoSystemInfo::new());
        let apps = Arc::new(ApplicationManager::new(
            sershi_platform::application_platform(),
            now_ms,
        ));
        let mut registry = ToolRegistry::default();
        builtin::register_all(&mut registry, system.clone())?;
        app_tools::register(&mut registry, apps.clone())?;
        let service = AssistantService::new(
            ToolExecutor::new(registry, PolicyEngine::new(Platform::current())),
            Box::new(KeywordIntentResolver),
            // Persisted grants arrive with the storage layer (ADR 0004).
            PermissionGrants::default(),
            now_ms,
        )
        // Understanding resolves names against the trusted catalog (Gate 3C).
        .with_applications(apps.clone());
        Ok(Self {
            service: Mutex::new(service),
            system,
            apps,
        })
    }

    pub fn with_service<R>(
        &self,
        f: impl FnOnce(&mut AssistantService) -> R,
    ) -> Result<R, IpcError> {
        let mut service = self.service.lock().map_err(|_| {
            IpcError::new(
                IpcErrorCode::Internal,
                "SERSHI's core is unavailable. Restart SERSHI.",
            )
        })?;
        Ok(f(&mut service))
    }
}

/// Drives a request to its outcome. Brain inference runs **outside** the
/// service lock (bounded by the engine's request timeout), and plans run one
/// step per lock, so the user can cancel in between. A brain decision or
/// step that is no longer current is discarded by the core.
pub fn drive(app: &AppHandle, mut progress: Progress) -> Option<CommandOutcome> {
    let runtime = app.state::<Runtime>();
    loop {
        progress = match progress {
            Progress::Done(outcome) => return Some(outcome),
            Progress::Think(ticket) => {
                let started = std::time::Instant::now();
                let result = ticket.brain.decide(&ticket.request);
                let ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
                runtime
                    .with_service(|s| {
                        s.complete_brain(ticket.id, result, ms, &mut |e| broadcast(app, e))
                    })
                    .ok()?
            }
            Progress::Step(report) => runtime
                .with_service(|s| s.advance_plan(report.id, &mut |e| broadcast(app, e)))
                .ok()?,
        };
    }
}

pub fn broadcast(app: &AppHandle, event: ServiceEvent) {
    // Emission only fails if the app is shutting down; nothing to recover.
    let _ = match event {
        ServiceEvent::State(snapshot) => {
            // The voice session indicator follows the working states.
            crate::voice::observe(app, snapshot.state);
            app.emit(STATE_EVENT, snapshot)
        }
        ServiceEvent::Activity(entry) => app.emit(ACTIVITY_EVENT, entry),
        ServiceEvent::Plan(report) => app.emit_to("main", PLAN_EVENT, report),
    };
}

/// Returns an assistant to idle after an outcome state has been visible long
/// enough to read, or expires a pending confirmation. Stale timers are
/// ignored via the snapshot revision (settle) or are idempotent (expiry).
pub fn schedule_settle(app: &AppHandle, snapshot: &AssistantSnapshot) {
    let hold = match snapshot.state {
        AssistantState::Success => Duration::from_millis(2_400),
        AssistantState::Warning | AssistantState::Error => Duration::from_millis(4_000),
        AssistantState::AwaitingConfirmation => {
            schedule_expiry(app);
            return;
        }
        AssistantState::WaitingForClarification => {
            schedule_clarification_expiry(app);
            return;
        }
        _ => return,
    };
    let app = app.clone();
    let revision = snapshot.revision;
    thread::spawn(move || {
        thread::sleep(hold);
        let runtime = app.state::<Runtime>();
        if let Ok(Some(settled)) =
            runtime.with_service(|s| s.apply_if_current(revision, AssistantEvent::Settle))
        {
            broadcast(&app, ServiceEvent::State(settled));
        }
    });
}

pub fn announce_ready(app: &AppHandle) {
    let runtime = app.state::<Runtime>();
    if let Ok(entry) = runtime.with_service(|s| {
        s.record(NewActivity::new(
            ActivityKind::SystemReady,
            "SERSHI core started",
        ))
    }) {
        broadcast(app, ServiceEvent::Activity(entry));
    }
}

/// Expires the pending confirmation shortly after its TTL, so the surface
/// closes and the Command Center updates without user action. Idempotent:
/// if the confirmation was decided or replaced, nothing is overdue.
fn schedule_expiry(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(CONFIRMATION_TTL_MS + 250));
        let runtime = app.state::<Runtime>();
        if let Ok(Some(outcome)) =
            runtime.with_service(|s| s.expire_confirmations(&mut |e| broadcast(&app, e)))
        {
            confirmation::expired(&app, &outcome);
        }
    });
}

/// Ends an unanswered question shortly after its TTL, so a stale question
/// can never be answered later and the surfaces return to idle. Idempotent:
/// an answered or newer question is not overdue.
fn schedule_clarification_expiry(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(CLARIFICATION_TTL_MS + 250));
        let runtime = app.state::<Runtime>();
        let _ = runtime.with_service(|s| s.expire_clarification(&mut |e| broadcast(&app, e)));
    });
}

/// Scans installed applications a few seconds after start-up, off the
/// critical path, so the first "open" is fast without slowing launch. Only
/// on platforms with application control.
pub fn warm_up_catalog(app: &AppHandle) {
    let apps = app.state::<Runtime>().apps.clone();
    if apps.status().state != CatalogState::NotScanned {
        return;
    }
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));
        if apps.status().state == CatalogState::NotScanned {
            let _ = apps.refresh();
        }
    });
}
