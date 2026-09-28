//! SERSHI desktop shell.
//!
//! This crate is deliberately thin: it owns windows, translates IPC calls into
//! `sershi-core` service calls, and broadcasts state to every window. It
//! contains no business rules and no direct OS actions — those live behind
//! the core's tool/policy pipeline and the platform adapters.

mod commands;
mod runtime;
mod surfaces;

use tauri::Manager;

pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let runtime = runtime::Runtime::new()?;
            app.manage(runtime);
            surfaces::place_companion(app.handle());
            runtime::announce_ready(app.handle());
            Ok(())
        })
        .on_window_event(surfaces::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_assistant_snapshot,
            commands::get_system_snapshot,
            commands::get_runtime_info,
            commands::list_activity,
            commands::submit_command,
            commands::summon_command_center,
            commands::dismiss_assistant,
            commands::preview_assistant_state,
            commands::quit_app,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = app {
        eprintln!("SERSHI failed to start: {error}");
        std::process::exit(1);
    }
}
