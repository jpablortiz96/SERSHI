//! SERSHI desktop shell.
//!
//! This crate is deliberately thin: it owns windows, OS integration (tray,
//! global shortcut, single instance), translates IPC calls into `sershi-core`
//! service calls, and broadcasts state to every window. It contains no
//! business rules and no direct OS actions — those live behind the core's
//! tool/policy pipeline and the platform adapters.

mod commands;
mod confirmation;
mod integration;
mod runtime;
mod surfaces;

use tauri::Manager;

pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch brings the running SERSHI forward
        // instead of starting another companion and tray icon.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            surfaces::show_all(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let runtime = runtime::Runtime::new()?;
            app.manage(runtime);
            app.manage(confirmation::ConfirmationSurface::default());
            let integration = integration::setup(app.handle());
            app.manage(integration);
            surfaces::place_companion(app.handle());
            runtime::announce_ready(app.handle());
            runtime::warm_up_catalog(app.handle());
            Ok(())
        })
        .on_window_event(surfaces::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_assistant_snapshot,
            commands::get_system_snapshot,
            commands::get_runtime_info,
            commands::list_activity,
            commands::submit_command,
            confirmation::get_confirmation_context,
            confirmation::decide_confirmation,
            commands::get_application_catalog,
            commands::refresh_application_catalog,
            commands::get_integration_status,
            commands::set_tray_labels,
            commands::summon_command_center,
            commands::hide_command_center,
            commands::dismiss_assistant,
            commands::preview_assistant_state,
            commands::quit_app,
        ])
        .build(tauri::generate_context!());

    match app {
        Ok(app) => app.run(|app, event| {
            // Any exit path (tray, Settings, OS shutdown): nothing pending
            // survives, and nothing can be approved on the way out.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                confirmation::shutdown(app);
            }
        }),
        Err(error) => {
            eprintln!("SERSHI failed to start: {error}");
            std::process::exit(1);
        }
    }
}
