//! IPC commands. Each is narrow and typed; there is no generic "execute"
//! command. Which window may call which command is declared in
//! `capabilities/*.json`.

use sershi_core::activity::ActivityEntry;
use sershi_core::assistant::{AssistantSnapshot, AssistantState};
use sershi_core::ipc::{
    ApplicationCatalogInfo, IntegrationStatus, IpcError, IpcErrorCode, RuntimeInfo, TrayLabels,
};
use sershi_core::platform::Platform;
use sershi_core::ports::SystemInfoProvider;
use sershi_core::service::{CommandOutcome, CommandRequest, ServiceEvent};
use sershi_core::shortcut::ShortcutChange;
use sershi_core::system::SystemSnapshot;
use tauri::{AppHandle, State};

use crate::confirmation;
use crate::integration;
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

/// Runs a command. Runs off the main thread (application discovery and
/// launching can take a moment) and broadcasts each state change as it
/// happens, so every surface shows Thinking → Planning → Executing live.
/// If the command needs approval, the trusted confirmation surface opens;
/// the outcome returned here never contains its id.
#[tauri::command(async)]
pub fn submit_command(
    app: AppHandle,
    runtime: State<'_, Runtime>,
    request: CommandRequest,
) -> Result<CommandOutcome, IpcError> {
    let (outcome, snapshot) = runtime.with_service(|s| {
        let outcome = s.submit(&request, &mut |event| broadcast(&app, event));
        (outcome, s.snapshot())
    })?;
    confirmation::sync(&app);
    schedule_settle(&app, &snapshot);
    Ok(outcome)
}

fn catalog_info(runtime: &Runtime) -> ApplicationCatalogInfo {
    ApplicationCatalogInfo {
        status: runtime.apps.status(),
        // Names and sources only, and only for Developer Mode.
        applications: if cfg!(debug_assertions) {
            runtime.apps.applications()
        } else {
            Vec::new()
        },
    }
}

#[tauri::command]
pub fn get_application_catalog(runtime: State<'_, Runtime>) -> ApplicationCatalogInfo {
    catalog_info(&runtime)
}

#[tauri::command(async)]
pub fn refresh_application_catalog(
    runtime: State<'_, Runtime>,
) -> Result<ApplicationCatalogInfo, IpcError> {
    runtime.apps.refresh().map_err(|_| {
        IpcError::new(
            IpcErrorCode::Unavailable,
            "The list of installed applications couldn't be read.",
        )
    })?;
    Ok(catalog_info(&runtime))
}

#[tauri::command]
pub fn get_integration_status(app: AppHandle) -> Result<IntegrationStatus, IpcError> {
    integration::state(&app)
        .map(|i| i.status())
        .ok_or_else(|| IpcError::new(IpcErrorCode::Unavailable, "Not ready yet."))
}

/// Changes the global summon shortcut (Command Center only). Registration
/// is attempted before anything is replaced; see `integration::apply_shortcut`.
/// Invocation only: a shortcut never carries authority.
#[tauri::command(async)]
pub fn set_global_shortcut(
    app: AppHandle,
    accelerator: String,
) -> Result<ShortcutChange, IpcError> {
    if accelerator.len() > 64 {
        return Err(IpcError::new(IpcErrorCode::NotAllowed, "Invalid shortcut."));
    }
    integration::apply_shortcut(&app, &accelerator)
        .ok_or_else(|| IpcError::new(IpcErrorCode::Unavailable, "Not ready yet."))
}

/// Localized tray labels. Presentation only: each item's action is fixed.
#[tauri::command]
pub fn set_tray_labels(app: AppHandle, labels: TrayLabels) -> Result<(), IpcError> {
    if !labels.is_valid() {
        return Err(IpcError::new(
            IpcErrorCode::NotAllowed,
            "Invalid tray labels.",
        ));
    }
    if let Some(integration) = integration::state(&app) {
        integration.set_tray_labels(&labels);
    }
    Ok(())
}

/// Shows the Command Center and focuses its command input.
#[tauri::command]
pub fn summon_command_center(app: AppHandle) {
    surfaces::summon(&app);
}

/// The Command Center's × button: hide, keep SERSHI running.
#[tauri::command]
pub fn hide_command_center(app: AppHandle) {
    surfaces::hide_command_center(&app);
}

/// Returns an attending or waiting assistant to idle (e.g. Escape).
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
    surfaces::quit(&app);
}
