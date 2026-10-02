//! The trusted confirmation surface: a dedicated, minimal-authority window
//! that is the only place a sensitive action can be approved.
//!
//! Rust owns its whole lifecycle. The core decides whether a confirmation is
//! pending; [`sync`] makes exactly one surface exist for it (or none). The
//! window loads only the app's own `confirmation.html`, cannot navigate
//! elsewhere, has no devtools in release builds, and is granted exactly two
//! commands (`capabilities/confirmation.json`). Closing it cancels.
//!
//! A separate WebView is not a security enclave: it shares the process and
//! origin with the other windows. Its value is a smaller capability surface,
//! no external or model-authored content and a single purpose (see
//! docs/SECURITY.md).
//!
//! REQUIRES_WINDOWS_VALIDATION: creation, focus, always-on-top and close
//! behaviour have only been exercised on Linux.

use std::sync::Mutex;

use sershi_core::confirmation::{
    ConfirmationId, ConfirmationRequest, SurfaceAction, SurfaceAssignment,
};
use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::service::{CommandOutcome, ConfirmationChoice, ConfirmationDecision, Progress};
use tauri::webview::Url;
use tauri::{
    AppHandle, Emitter, Manager, UserAttentionType, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, Window,
};

use crate::runtime::{Runtime, broadcast, drive, schedule_settle};
use crate::surfaces::MAIN;
use crate::{permissions, voice};

/// The confirmation window's label (stable: one surface at a time).
pub const CONFIRMATION: &str = "confirmation";

/// Carries a `CommandOutcome` decided outside the Command Center (approval,
/// cancellation, expiry) to the Command Center's transcript.
pub const OUTCOME_EVENT: &str = "sershi://command-outcome";

const WIDTH: f64 = 440.0;
const HEIGHT: f64 = 320.0;

/// Which confirmation the surface shows (managed state).
#[derive(Debug, Default)]
pub struct ConfirmationSurface {
    assignment: Mutex<SurfaceAssignment>,
}

fn assignment<R>(app: &AppHandle, f: impl FnOnce(&mut SurfaceAssignment) -> R) -> Option<R> {
    let surface = app.try_state::<ConfirmationSurface>()?;
    let mut guard = surface.assignment.lock().ok()?;
    Some(f(&mut guard))
}

fn not_allowed() -> IpcError {
    IpcError::new(
        IpcErrorCode::NotAllowed,
        "That request is no longer waiting for approval.",
    )
}

/// Only the confirmation window may use the confirmation commands. The
/// capability files already enforce this; the label check is defence in
/// depth should a capability ever be misconfigured.
fn require_surface(window: &Window) -> Result<(), IpcError> {
    if window.label() == CONFIRMATION {
        Ok(())
    } else {
        Err(IpcError::new(
            IpcErrorCode::NotAllowed,
            "Only the confirmation window can do this.",
        ))
    }
}

/// Aligns the surface with the core's pending confirmation: open it, point
/// it at a replacement, or destroy it. Must run off the main thread when a
/// confirmation may have been created (window creation from a synchronous
/// command deadlocks on Windows); callers that create confirmations are
/// `async` commands.
pub fn sync(app: &AppHandle) {
    let pending = app
        .state::<Runtime>()
        .with_service(|s| s.pending_confirmation().map(|r| r.id))
        .ok()
        .flatten();
    let shown = match assignment(app, |a| a.reconcile(pending.as_ref())) {
        Some(SurfaceAction::Open(_)) => open(app),
        Some(SurfaceAction::Replace(_)) => {
            // Reload so the surface fetches the new confirmation and its
            // input guard re-arms (no click aimed at the old one carries over).
            if let Some(window) = app.get_webview_window(CONFIRMATION) {
                let _ = window.reload();
                present(&window);
                true
            } else {
                open(app)
            }
        }
        Some(SurfaceAction::Close) => {
            destroy(app);
            true
        }
        Some(SurfaceAction::Keep) => {
            // Health check (Gate 4.1.1): a pending confirmation must have a
            // window. One that vanished is shown again.
            pending.is_none() || app.get_webview_window(CONFIRMATION).is_some() || open(app)
        }
        None => true,
    };
    if !shown {
        withdraw(app);
    }
}

/// The trusted window could not be shown (Gate 4.1.1): an approval nobody
/// can see must not wait. The core withdraws it — nothing it covered runs
/// — and the conversation says so.
fn withdraw(app: &AppHandle) {
    assignment(app, SurfaceAssignment::release);
    let runtime = app.state::<Runtime>();
    let outcome = runtime
        .with_service(|s| s.withdraw_confirmation(&mut |e| broadcast(app, e)))
        .ok()
        .flatten();
    if let Some(outcome) = outcome {
        eprintln!("SERSHI: the confirmation window could not be shown; the approval was withdrawn");
        emit_outcome(app, &outcome);
    }
    if let Ok(snapshot) = runtime.with_service(|s| s.snapshot()) {
        schedule_settle(app, &snapshot);
    }
    voice::confirmation_resolved(app);
}

/// Creates (or re-presents) the surface. False if no window could be shown.
fn open(app: &AppHandle) -> bool {
    if let Some(existing) = app.get_webview_window(CONFIRMATION) {
        let _ = existing.reload();
        present(&existing);
        return true;
    }
    let built = WebviewWindowBuilder::new(
        app,
        CONFIRMATION,
        WebviewUrl::App("confirmation.html".into()),
    )
    .title("SERSHI")
    .inner_size(WIDTH, HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .always_on_top(true)
    .center()
    .focused(true)
    .background_color(tauri::window::Color(7, 8, 11, 255))
    .devtools(cfg!(debug_assertions))
    .on_navigation(is_own_page)
    .build();
    match built {
        Ok(window) => {
            present(&window);
            true
        }
        Err(error) => {
            eprintln!("SERSHI: confirmation window unavailable: {error}");
            false
        }
    }
}

/// The surface may only ever show SERSHI's own bundled page.
fn is_own_page(url: &Url) -> bool {
    match (url.scheme(), url.host_str()) {
        ("tauri", Some("localhost")) => true,
        ("http" | "https", Some("tauri.localhost")) => true,
        // `pnpm dev` serves the frontend from the Vite dev server.
        ("http", Some("localhost")) => cfg!(debug_assertions) && url.port() == Some(1420),
        _ => false,
    }
}

fn present(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    if !window.is_focused().unwrap_or(true) {
        let _ = window.request_user_attention(Some(UserAttentionType::Critical));
    }
}

fn destroy(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CONFIRMATION) {
        let _ = window.destroy();
    }
}

/// Brings a visible surface back to the front (e.g. SERSHI was summoned
/// while an approval is pending).
pub fn focus_if_open(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CONFIRMATION) {
        present(&window);
    }
}

fn emit_outcome(app: &AppHandle, outcome: &CommandOutcome) {
    // A permission decision belongs to Settings, not to the conversation.
    if permissions::handled(app, outcome) {
        return;
    }
    let _ = app.emit_to(MAIN, OUTCOME_EVENT, outcome);
}

/// Runs a decision for the surface's assigned confirmation and cleans up.
fn apply(app: &AppHandle, id: ConfirmationId, choice: ConfirmationChoice) {
    let runtime = app.state::<Runtime>();
    let decision = ConfirmationDecision {
        confirmation_id: id,
        decision: choice,
    };
    if let Ok(outcome) =
        runtime.with_service(|s| s.decide(&decision, &mut |event| broadcast(app, event)))
    {
        emit_outcome(app, &outcome);
        // An approved plan step: the rest of the plan continues, each step
        // with its own policy (a later sensitive step opens a new
        // confirmation of its own).
        let outcome = match outcome.plan.as_ref().filter(|p| !p.done) {
            Some(report) => drive(app, Progress::Step(report.clone())),
            None => None,
        };
        if let Some(outcome) = outcome {
            emit_outcome(app, &outcome);
        }
        if let Ok(snapshot) = runtime.with_service(|s| s.snapshot()) {
            schedule_settle(app, &snapshot);
        }
    }
    sync(app);
    // A voice session paused for this approval listens again.
    voice::confirmation_resolved(app);
}

/// The surface asks what it should show: its assigned confirmation only.
#[tauri::command]
pub fn get_confirmation_context(
    window: Window,
    runtime: tauri::State<'_, Runtime>,
) -> Result<ConfirmationRequest, IpcError> {
    require_surface(&window)?;
    let app = window.app_handle();
    let request = runtime
        .with_service(|s| s.pending_confirmation())?
        .ok_or_else(not_allowed)?;
    if assignment(app, |a| a.authorizes(&request.id)) != Some(true) {
        return Err(not_allowed());
    }
    Ok(request)
}

/// The human's decision. Accepted only from the confirmation window, only
/// for the confirmation it was opened for; carries only the id and the
/// choice — the stored call is what runs.
#[tauri::command(async)]
pub fn decide_confirmation(window: Window, decision: ConfirmationDecision) -> Result<(), IpcError> {
    require_surface(&window)?;
    let app = window.app_handle().clone();
    // Check and release in one step: only the first decision for the
    // assigned confirmation gets through, and closing the surface afterwards
    // must not cancel anything. (The core's `take` is one-time as well.)
    let released = assignment(&app, |a| {
        a.authorizes(&decision.confirmation_id)
            .then(|| a.release())
            .flatten()
    })
    .flatten();
    if released.is_none() {
        sync(&app);
        return Err(not_allowed());
    }
    let _ = window.hide();
    apply(&app, decision.confirmation_id, decision.decision);
    // Gate 4.1.1 root cause: applying the decision may continue a plan whose
    // next step needs its own approval; `sync` (inside `apply`) has already
    // pointed this window at it and shown it again. Destroying the window
    // unconditionally here used to tear that new approval's window down,
    // leaving it pending and invisible. Destroy only when nothing waits.
    destroy_if_idle(&app);
    Ok(())
}

/// Destroys the surface if no confirmation is pending any more.
fn destroy_if_idle(app: &AppHandle) {
    let pending = app
        .state::<Runtime>()
        .with_service(|s| s.pending_confirmation().is_some())
        .unwrap_or(false);
    if !pending {
        assignment(app, SurfaceAssignment::release);
        destroy(app);
    }
}

/// The user closed the surface (× or Alt+F4): that is a cancellation, never
/// an approval.
pub fn on_close_requested(app: &AppHandle) {
    if let Some(Some(id)) = assignment(app, SurfaceAssignment::release) {
        apply(app, id, ConfirmationChoice::Cancel);
    }
    destroy_if_idle(app);
}

/// An expired confirmation: report it to the Command Center and close the
/// surface.
pub fn expired(app: &AppHandle, outcome: &CommandOutcome) {
    emit_outcome(app, outcome);
    sync(app);
    voice::confirmation_resolved(app);
}

/// Quitting SERSHI: cancel whatever is pending and destroy the surface.
/// Nothing is persisted, so nothing can be restored after a restart.
pub fn shutdown(app: &AppHandle) {
    assignment(app, SurfaceAssignment::release);
    let runtime = app.state::<Runtime>();
    let _ = runtime.with_service(|s| s.dismiss(&mut |_| {}));
    destroy(app);
}

/// Reports a pending approval that was cancelled from the Command Center
/// (Escape, hiding it, a new request).
pub fn cancelled(app: &AppHandle, outcome: Option<CommandOutcome>) {
    if let Some(outcome) = outcome {
        emit_outcome(app, &outcome);
    }
    sync(app);
}
