//! Declares every IPC command so Tauri generates a per-command permission.
//! Windows only receive the commands their capability file grants
//! (see `capabilities/`), so the companion cannot call Command Center APIs.

const COMMANDS: &[&str] = &[
    "get_assistant_snapshot",
    "get_system_snapshot",
    "get_runtime_info",
    "list_activity",
    "submit_command",
    "get_confirmation_context",
    "decide_confirmation",
    "get_application_catalog",
    "refresh_application_catalog",
    "get_integration_status",
    "set_tray_labels",
    "set_global_shortcut",
    "summon_command_center",
    "hide_command_center",
    "dismiss_assistant",
    "preview_assistant_state",
    "quit_app",
    "get_voice_status",
    "configure_voice",
    "start_voice_capture",
    "stop_voice_capture",
    "cancel_voice_capture",
    "speak_reply",
    "stop_speaking",
    "download_voice_model",
    "cancel_voice_model_download",
    "get_semantic_status",
    "configure_semantic",
    "download_semantic_model",
    "cancel_semantic_model_download",
];

fn main() {
    // GPU builds delay-load the Vulkan loader so SERSHI starts on machines
    // without a GPU driver; `sershi_platform` checks for it first.
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if windows && std::env::var_os("CARGO_FEATURE_VULKAN").is_some() {
        println!("cargo:rustc-link-arg=/DELAYLOAD:vulkan-1.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("tauri build failed: {error:#}");
        std::process::exit(1);
    }
}
