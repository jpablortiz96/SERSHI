//! IPC commands. Each is narrow and typed; there is no generic "execute"
//! command. Which window may call which command is declared in
//! `capabilities/*.json`.

use sershi_core::activity::ActivityEntry;
use sershi_core::assistant::{AssistantEvent, AssistantSnapshot, AssistantState};
use sershi_core::ipc::{IpcError, IpcErrorCode, RuntimeInfo};
use sershi_core::platform::Platform;
use sershi_core::ports::SystemInfoProvider;
use sershi_core::service::{CommandOutcome, CommandRequest, ServiceEvent};
use sershi_core::system::SystemSnapshot;
use tauri::{AppHandle, State};

use crate::runtime::{Runtime, broadcast, schedule_settle};
use crate::surfaces;

const MAX_ACTIVITY_PAGE: u32 = 100;

#[tauri::command]
pub fn get_assistant_snapshot(runtime: State<'_, Runtime>) -> Result<AssistantSnapshot, IpcError> {
    runtime.with_service(|s| s.snapshot())
}

/// Read-only telemetry for the Command Center. This is the user looking at
/// their own machine, not the agent acting, so it bypasses the tool pipeline
/// and is not recorded as activity (it is polled).
#[tauri::command(async)]
pub fn get_system_snapshot(runtime: State<'_, Runtime>) -> Result<SystemSnapshot, IpcError> {
    runtime.system.snapshot().map_err(|_| {
        IpcError::new(
            IpcErrorCode::Unavailable,
            "System telemetry isn't available right now.",
        )
    })
}

#[tauri::command]
pub fn get_runtime_info(runtime: State<'_, Runtime>) -> Result<RuntimeInfo, IpcError> {
    let tools = runtime.with_service(|s| s.tool_definitions())?;
    Ok(RuntimeInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: Platform::current(),
        developer_build: cfg!(debug_assertions),
        capabilities: sershi_platform::capabilities(),
        tools,
    })
}

#[tauri::command]
pub fn list_activity(
    runtime: State<'_, Runtime>,
    limit: u32,
) -> Result<Vec<ActivityEntry>, IpcError> {
    let limit = limit.clamp(1, MAX_ACTIVITY_PAGE) as usize;
    runtime.with_service(|s| s.recent_activity(limit))
}

#[tauri::command(async)]
pub fn submit_command(
    app: AppHandle,
    runtime: State<'_, Runtime>,
    request: CommandRequest,
) -> Result<CommandOutcome, IpcError> {
    let mut events = Vec::new();
    let (outcome, snapshot) = runtime.with_service(|s| {
        let outcome = s.submit(&request, &mut |e| events.push(e));
        (outcome, s.snapshot())
    })?;
    // Broadcast outside the lock so listeners can call back into the core.
    for event in events {
        broadcast(&app, event);
    }
    schedule_settle(&app, &snapshot);
    Ok(outcome)
}

/// Shows the Command Center and marks the assistant as attending.
#[tauri::command]
pub fn summon_command_center(app: AppHandle, runtime: State<'_, Runtime>) -> Result<(), IpcError> {
    surfaces::show_command_center(&app);
    let activated = runtime.with_service(|s| {
        matches!(
            s.snapshot().state,
            AssistantState::Idle | AssistantState::Sleeping
        )
        .then(|| s.apply(AssistantEvent::Activate).ok())
        .flatten()
    })?;
    if let Some(snapshot) = activated {
        broadcast(&app, ServiceEvent::State(snapshot));
    }
    Ok(())
}

/// Returns an attending assistant to idle (e.g. Escape, or hiding the window).
#[tauri::command]
pub fn dismiss_assistant(app: AppHandle, runtime: State<'_, Runtime>) -> Result<(), IpcError> {
    surfaces::dismiss(&app, &runtime)
}

/// Developer Mode: preview a state's visuals without changing behaviour.
#[tauri::command]
pub fn preview_assistant_state(
    app: AppHandle,
    runtime: State<'_, Runtime>,
    state: Option<AssistantState>,
) -> Result<AssistantSnapshot, IpcError> {
    if !cfg!(debug_assertions) {
        return Err(IpcError::new(
            IpcErrorCode::NotAllowed,
            "State preview is only available in developer builds.",
        ));
    }
    let snapshot = runtime.with_service(|s| s.set_preview(state))?;
    broadcast(&app, ServiceEvent::State(snapshot.clone()));
    Ok(snapshot)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
