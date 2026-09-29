//! Window behaviour for the two SERSHI surfaces.
//!
//! REQUIRES_WINDOWS_VALIDATION: companion placement over the taskbar work
//! area, transparency, always-on-top, hide-on-close and foreground activation
//! (summon) have only been exercised on Linux (X11 under Xvfb).

use std::thread;
use std::time::Duration;

use sershi_core::assistant::{AssistantEvent, AssistantState};
use sershi_core::ipc::{IpcError, PresenceUpdate};
use sershi_core::service::ServiceEvent;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, UserAttentionType, Window,
    WindowEvent,
};

use crate::confirmation::{self, CONFIRMATION};
use crate::runtime::{Runtime, broadcast, schedule_settle};

/// Asks the Command Center to focus its command input.
pub const FOCUS_COMMAND_EVENT: &str = "sershi://focus-command";
/// Tells the companion whether the Command Center is on screen (contextual
/// presence; carries no authority).
pub const PRESENCE_EVENT: &str = "sershi://presence";

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

/// Brings the Command Center forward from any state: hidden, minimized,
/// behind other windows or already visible. If Windows refuses to move focus
/// (foreground lock), the taskbar button flashes instead of fighting it.
pub fn show_command_center(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    let _ = window.show();
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.set_focus();
    announce_presence(app, true);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        if !window.is_focused().unwrap_or(true) {
            let _ = window.request_user_attention(Some(UserAttentionType::Informational));
        }
    });
}

/// Lets the companion adapt its presence to the Command Center's visibility.
pub fn announce_presence(app: &AppHandle, command_center_visible: bool) {
    let _ = app.emit_to(
        COMPANION,
        PRESENCE_EVENT,
        PresenceUpdate {
            command_center_visible,
        },
    );
}

/// Summon: show the Command Center, focus the command input, and mark the
/// assistant as attending. Used by the companion, tray, shortcut and a
/// second launch of SERSHI. If an approval is pending, its trusted surface
/// comes forward instead of the command input; summoning never decides it.
pub fn summon(app: &AppHandle) {
    show_command_center(app);
    if app.get_webview_window(CONFIRMATION).is_some() {
        confirmation::focus_if_open(app);
    } else {
        let _ = app.emit_to(MAIN, FOCUS_COMMAND_EVENT, ());
    }
    let runtime = app.state::<Runtime>();
    if let Ok(Some(snapshot)) = runtime.with_service(|s| {
        matches!(
            s.snapshot().state,
            AssistantState::Idle | AssistantState::Sleeping
        )
        .then(|| s.apply(AssistantEvent::Activate).ok())
        .flatten()
    }) {
        broadcast(app, ServiceEvent::State(snapshot));
    }
}

/// Tray "Open SERSHI": both surfaces visible, Command Center focused.
pub fn show_all(app: &AppHandle) {
    if let Some(companion) = app.get_webview_window(COMPANION) {
        let _ = companion.show();
    }
    summon(app);
}

/// Tray "Hide SERSHI": SERSHI keeps running in the tray only.
pub fn hide_all(app: &AppHandle) {
    hide_command_center(app);
    if let Some(companion) = app.get_webview_window(COMPANION) {
        let _ = companion.hide();
    }
}

/// Hides the Command Center (the × button). SERSHI stays alive in the
/// companion, tray and shortcut; pending approvals are cancelled.
pub fn hide_command_center(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.hide();
    }
    announce_presence(app, false);
    let _ = dismiss(app, &app.state::<Runtime>());
}

/// Returns an attending or waiting assistant to idle, cancelling any pending
/// approval.
pub fn dismiss(app: &AppHandle, runtime: &Runtime) -> Result<(), IpcError> {
    let (cancelled, snapshot) = runtime.with_service(|s| {
        let cancelled = s.dismiss(&mut |event| broadcast(app, event));
        (cancelled, s.snapshot())
    })?;
    confirmation::cancelled(app, cancelled);
    schedule_settle(app, &snapshot);
    Ok(())
}

/// Closing the Command Center hides it; SERSHI keeps living in the companion
/// and tray. Quitting is explicit (Settings or tray). Closing the
/// confirmation surface cancels its confirmation.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    match window.label() {
        MAIN => {
            api.prevent_close();
            hide_command_center(window.app_handle());
        }
        CONFIRMATION => {
            api.prevent_close();
            confirmation::on_close_requested(window.app_handle());
        }
        _ => {}
    }
}

/// Quit: pending approvals are cancelled and the surface destroyed before
/// the process exits, so nothing can execute.
pub fn quit(app: &AppHandle) {
    confirmation::shutdown(app);
    app.exit(0);
}
