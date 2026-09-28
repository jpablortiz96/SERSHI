//! Declares every IPC command so Tauri generates a per-command permission.
//! Windows only receive the commands their capability file grants
//! (see `capabilities/`), so the companion cannot call Command Center APIs.

const COMMANDS: &[&str] = &[
    "get_assistant_snapshot",
    "get_system_snapshot",
    "get_runtime_info",
    "list_activity",
    "submit_command",
    "summon_command_center",
    "dismiss_assistant",
    "preview_assistant_state",
    "quit_app",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("tauri build failed: {error:#}");
        std::process::exit(1);
    }
}
