//! IPC commands. Each is narrow and typed; there is no generic "execute"
//! command. Which window may call which command is declared in
//! `capabilities/*.json`.

use sershi_core::activity::ActivityEntry;
use sershi_core::assistant::{AssistantSnapshot, AssistantState};
use sershi_core::ipc::{
    ApplicationCatalogInfo, IntegrationStatus, IpcError, IpcErrorCode, RuntimeInfo, TrayLabels,
};
use sershi_core::permission::{ConfigurablePermission, PermissionSetting, PermissionStatus};
use sershi_core::platform::Platform;
use sershi_core::ports::SystemInfoProvider;
use sershi_core::service::{CommandOutcome, CommandRequest, PermissionChange, ServiceEvent};
use sershi_core::shortcut::ShortcutChange;
use sershi_core::system::SystemSnapshot;
use sershi_core::understanding::status::{SemanticSettings, SemanticStatus};
use sershi_core::voice::session::VoiceSessionStatus;
use sershi_core::voice::{CaptureStart, VoiceSettings, VoiceStatus};
use tauri::{AppHandle, State};

use crate::confirmation;
use crate::integration;
use crate::local_model::{self, Role};
use crate::permissions;
use crate::runtime::{Runtime, broadcast, drive, schedule_settle};
use crate::surfaces;
use crate::voice;

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
    // A typed command takes over: SERSHI stops listening and talking.
    voice::interrupt(&app);
    let progress =
        runtime.with_service(|s| s.begin_request(&request, &mut |event| broadcast(&app, event)))?;
    // The brain model and plan steps run without holding the service.
    let outcome = drive(&app, progress)
        .ok_or_else(|| IpcError::new(IpcErrorCode::Internal, "SERSHI's core is unavailable."))?;
    let snapshot = runtime.with_service(|s| s.snapshot())?;
    confirmation::sync(&app);
    schedule_settle(&app, &snapshot);
    // In a voice session, typing is a turn too: it listens again after.
    voice::after_typed(&app, &outcome);
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

// ── Voice (Command Center only; docs/VOICE.md) ────────────────────────────
//
// Voice is input and output, never authorization: none of these commands
// can approve, read a confirmation or change a permission. A transcript is
// submitted through the same core path as `submit_command`.

/// Microphones, voices, models and whether SERSHI is listening or speaking.
#[tauri::command(async)]
pub fn get_voice_status(app: AppHandle) -> Result<VoiceStatus, IpcError> {
    voice::status(&app)
}

/// Stores the user's voice preferences (device, language, voice, model).
#[tauri::command(async)]
pub fn configure_voice(app: AppHandle, settings: VoiceSettings) -> Result<VoiceStatus, IpcError> {
    voice::configure(&app, settings)
}

/// Push-to-talk: opens the microphone (never on its own, never in the
/// background).
#[tauri::command(async)]
pub fn start_voice_capture(app: AppHandle) -> Result<CaptureStart, IpcError> {
    voice::start_capture(&app)
}

/// Stops listening and transcribes what was said.
/// Starts a hands-free voice session (Gate 4.1). Explicit: there is no
/// wake word and no listening outside a session.
#[tauri::command(async)]
pub fn start_voice_session(app: AppHandle) -> Result<CaptureStart, IpcError> {
    voice::start_session(&app)
}

#[tauri::command(async)]
pub fn stop_voice_session(app: AppHandle) {
    voice::stop_session(&app);
}

#[tauri::command]
pub fn get_voice_session(app: AppHandle) -> Option<VoiceSessionStatus> {
    voice::session_status(&app)
}

#[tauri::command]
pub fn get_permission_settings(app: AppHandle) -> Result<Vec<PermissionStatus>, IpcError> {
    permissions::current(&app)
}

/// Settings › Security. Making a permission less restrictive opens the
/// trusted confirmation window; only its decision applies the change.
#[tauri::command(async)]
pub fn request_permission_change(
    app: AppHandle,
    permission: ConfigurablePermission,
    setting: PermissionSetting,
) -> Result<PermissionChange, IpcError> {
    permissions::request_change(&app, permission, setting)
}

#[tauri::command]
pub fn stop_voice_capture(app: AppHandle) {
    voice::stop_capture(&app);
}

/// Stops listening and discards what was captured.
#[tauri::command]
pub fn cancel_voice_capture(app: AppHandle) {
    voice::cancel_capture(&app);
}

/// Speaks a reply the Command Center phrased (output only).
#[tauri::command(async)]
pub fn speak_reply(app: AppHandle, text: String, language: Option<String>) -> Result<(), IpcError> {
    voice::speak(&app, &text, language.as_deref())
}

#[tauri::command(async)]
pub fn stop_speaking(app: AppHandle) {
    voice::stop_speaking(&app);
}

/// Downloads a speech model from SERSHI's fixed catalog (user-initiated).
#[tauri::command(async)]
pub fn download_voice_model(app: AppHandle, model: String) -> Result<(), IpcError> {
    if model.len() > 64 {
        return Err(IpcError::new(
            IpcErrorCode::NotAllowed,
            "Unknown speech model.",
        ));
    }
    voice::download_model(&app, &model)
}

#[tauri::command]
pub fn cancel_voice_model_download(app: AppHandle) {
    voice::cancel_download(&app);
}

// ── Natural command understanding (Command Center only; docs/SEMANTIC.md) ─
//
// The local semantic model only interprets text. None of these commands can
// run a tool, approve anything or reach the model's input or output.

/// The semantic model's installation and runtime state.
#[tauri::command(async)]
pub fn get_semantic_status(app: AppHandle) -> Result<SemanticStatus, IpcError> {
    local_model::status(&app, Role::Semantic)
}

/// Turns natural understanding with the local model on or off.
#[tauri::command(async)]
pub fn configure_semantic(
    app: AppHandle,
    settings: SemanticSettings,
) -> Result<SemanticStatus, IpcError> {
    local_model::configure(&app, Role::Semantic, settings)
}

/// Downloads the semantic model from SERSHI's fixed catalog (user-initiated).
#[tauri::command(async)]
pub fn download_semantic_model(app: AppHandle) -> Result<(), IpcError> {
    local_model::download(&app, Role::Semantic)
}

#[tauri::command]
pub fn cancel_semantic_model_download(app: AppHandle) {
    local_model::cancel_download(&app, Role::Semantic);
}

// ── The local Agent Brain (Command Center only; docs/AGENT_BRAIN.md) ───────
//
// The brain decides and proposes; it never executes or approves. None of
// these commands can run a tool or reach the brain's input or output.

#[tauri::command(async)]
pub fn get_brain_status(app: AppHandle) -> Result<SemanticStatus, IpcError> {
    local_model::status(&app, Role::Brain)
}

/// Turns the local Agent Brain on or off (once installed).
#[tauri::command(async)]
pub fn configure_brain(
    app: AppHandle,
    settings: SemanticSettings,
) -> Result<SemanticStatus, IpcError> {
    local_model::configure(&app, Role::Brain, settings)
}

/// Downloads the brain model from SERSHI's fixed catalog (user-initiated;
/// refused when the disk cannot hold it with a safety margin).
#[tauri::command(async)]
pub fn download_brain_model(app: AppHandle) -> Result<(), IpcError> {
    local_model::download(&app, Role::Brain)
}

#[tauri::command]
pub fn cancel_brain_model_download(app: AppHandle) {
    local_model::cancel_download(&app, Role::Brain);
}

/// Starts a new conversation: forgets the session context (recent
/// applications, turns, open questions). Preferences are kept; a pending
/// approval or running plan is cancelled. Grants nothing.
#[tauri::command]
pub fn reset_conversation(app: AppHandle, runtime: State<'_, Runtime>) -> Result<(), IpcError> {
    runtime.with_service(|s| s.reset_conversation(&mut |event| broadcast(&app, event)))?;
    confirmation::sync(&app);
    Ok(())
}
