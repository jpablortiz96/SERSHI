//! Window behaviour for the two SERSHI surfaces.
//!
//! REQUIRES_WINDOWS_VALIDATION: companion placement over the taskbar work
//! area, transparency, always-on-top and hide-on-close have only been
//! exercised on Linux (X11 under Xvfb).

use sershi_core::assistant::{AssistantEvent, AssistantState};
use sershi_core::ipc::IpcError;
use sershi_core::service::ServiceEvent;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, Window, WindowEvent};

use crate::runtime::{Runtime, broadcast};

pub const MAIN: &str = "main";
pub const COMPANION: &str = "companion";

/// Logical-pixel gap between the companion and the work-area edge.
const COMPANION_MARGIN: f64 = 24.0;
/// Logical size of the companion window; keep in sync with `tauri.conf.json`.
const COMPANION_SIZE: f64 = 176.0;

/// Places the companion at the bottom-right of the primary monitor's work
/// area (the screen minus the taskbar).
///
/// Uses the configured size rather than `outer_size()`: during setup the
/// window may not be realized yet and report 0×0 (observed on X11), which
/// would push the companion off-screen.
pub fn place_companion(app: &AppHandle) {
    let Some(window) = app.get_webview_window(COMPANION) else {
        return;
    };
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let _ = window.set_size(LogicalSize::new(COMPANION_SIZE, COMPANION_SIZE));
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let extent = ((COMPANION_SIZE + COMPANION_MARGIN) * scale).round() as i32;
    let x = area.position.x + area.size.width as i32 - extent;
    let y = area.position.y + area.size.height as i32 - extent;
    let _ = window.set_position(PhysicalPosition::new(
        x.max(area.position.x),
        y.max(area.position.y),
    ));
}

pub fn show_command_center(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn dismiss(app: &AppHandle, runtime: &Runtime) -> Result<(), IpcError> {
    let dismissed = runtime.with_service(|s| {
        (s.snapshot().state == AssistantState::Awake)
            .then(|| s.apply(AssistantEvent::Dismiss).ok())
            .flatten()
    })?;
    if let Some(snapshot) = dismissed {
        broadcast(app, ServiceEvent::State(snapshot));
    }
    Ok(())
}

/// Closing the Command Center hides it; SERSHI keeps living in the companion.
/// Quitting is an explicit action in the Command Center.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let _ = window.hide();
        let app = window.app_handle();
        let _ = dismiss(app, &app.state::<Runtime>());
    }
}
