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
mod semantic;
mod surfaces;
mod voice;

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
            // Speech models live in the per-user app-data directory
            // (%LOCALAPPDATA%\dev.sershi.desktop\models\stt on Windows).
            let models = app.path().app_local_data_dir()?.join("models");
            app.manage(voice::Voice::new(models.join("stt")));
            // The optional local semantic model (models\semantic).
            app.manage(semantic::Semantic::new(models.join("semantic")));
            app.manage(confirmation::ConfirmationSurface::default());
            let integration = integration::setup(app.handle());
            app.manage(integration);
            integration::schedule_default_shortcut(app.handle());
            surfaces::place_companion(app.handle());
            runtime::announce_ready(app.handle());
            runtime::warm_up_catalog(app.handle());
            semantic::setup(app.handle());
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
            commands::set_global_shortcut,
            commands::summon_command_center,
            commands::hide_command_center,
            commands::dismiss_assistant,
            commands::preview_assistant_state,
            commands::quit_app,
            commands::get_voice_status,
            commands::configure_voice,
            commands::start_voice_capture,
            commands::stop_voice_capture,
            commands::cancel_voice_capture,
            commands::speak_reply,
            commands::stop_speaking,
            commands::download_voice_model,
            commands::cancel_voice_model_download,
            commands::get_semantic_status,
            commands::configure_semantic,
            commands::download_semantic_model,
            commands::cancel_semantic_model_download,
        ])
        .build(tauri::generate_context!());

    match app {
        Ok(app) => app.run(|app, event| {
            // Any exit path (tray, Settings, OS shutdown): nothing pending
            // survives, and nothing can be approved on the way out.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                confirmation::shutdown(app);
                // Release the microphone and speaker; stop downloads.
                voice::shutdown(app);
                semantic::shutdown(app);
            }
        }),
        Err(error) => {
            eprintln!("SERSHI failed to start: {error}");
            std::process::exit(1);
        }
    }
}
