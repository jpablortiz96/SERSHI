//! Process-wide state and event broadcasting.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sershi_core::activity::{ActivityKind, NewActivity};
use sershi_core::assistant::{AssistantEvent, AssistantSnapshot, AssistantState};
use sershi_core::builtin::{self, BuiltinError};
use sershi_core::executor::ToolExecutor;
use sershi_core::intent::KeywordIntentResolver;
use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::permission::PermissionGrants;
use sershi_core::platform::Platform;
use sershi_core::policy::PolicyEngine;
use sershi_core::service::{AssistantService, ServiceEvent};
use sershi_core::tool::ToolRegistry;
use sershi_platform::SysinfoSystemInfo;
use tauri::{AppHandle, Emitter, Manager};

/// Event carrying an `AssistantSnapshot` to every window.
pub const STATE_EVENT: &str = "sershi://assistant-state";
/// Event carrying a new `ActivityEntry` to every window.
pub const ACTIVITY_EVENT: &str = "sershi://activity";

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
}

impl Runtime {
    pub fn new() -> Result<Self, BuiltinError> {
        let system = Arc::new(SysinfoSystemInfo::new());
        let mut registry = ToolRegistry::default();
        builtin::register_all(&mut registry, system.clone())?;
        let service = AssistantService::new(
            ToolExecutor::new(registry, PolicyEngine::new(Platform::current())),
            Box::new(KeywordIntentResolver),
            // Persisted grants arrive with the storage layer (ADR 0004).
            PermissionGrants::default(),
            now_ms,
        );
        Ok(Self {
            service: Mutex::new(service),
            system,
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

pub fn broadcast(app: &AppHandle, event: ServiceEvent) {
    // Emission only fails if the app is shutting down; nothing to recover.
    let _ = match event {
        ServiceEvent::State(snapshot) => app.emit(STATE_EVENT, snapshot),
        ServiceEvent::Activity(entry) => app.emit(ACTIVITY_EVENT, entry),
    };
}

/// Returns an assistant to idle after an outcome state has been visible long
/// enough to read. Stale timers are ignored via the snapshot revision.
pub fn schedule_settle(app: &AppHandle, snapshot: &AssistantSnapshot) {
    let hold = match snapshot.state {
        AssistantState::Success => Duration::from_millis(2_400),
        AssistantState::Warning | AssistantState::Error => Duration::from_millis(4_000),
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
