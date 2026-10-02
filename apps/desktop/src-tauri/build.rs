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
    "get_brain_status",
    "configure_brain",
    "download_brain_model",
    "cancel_brain_model_download",
    "reset_conversation",
    "start_voice_session",
    "stop_voice_session",
    "get_voice_session",
    "get_permission_settings",
    "request_permission_change",
];

/// Installer builds bundle the inference engine (`tauri.bundle.conf.json`).
/// Its SHA-256 is compiled in, so SERSHI refuses to start an engine that is
/// not the one it shipped with. Ordinary builds embed nothing (the engine
/// is the developer's own build next to the executable).
fn embed_engine_hash() {
    use sha2::{Digest, Sha256};
    println!("cargo:rerun-if-env-changed=TAURI_CONFIG");
    let bundling = std::env::var("TAURI_CONFIG").is_ok_and(|c| c.contains("externalBin"));
    let target = std::env::var("TARGET").unwrap_or_default();
    let ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let path = format!("binaries/sershi-semantic-{target}{ext}");
    println!("cargo:rerun-if-changed={path}");
    if !bundling {
        return;
    }
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("bundled engine {path} is missing (run scripts/stage-sidecars.mjs)");
        std::process::exit(1);
    };
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    println!("cargo:rustc-env=SERSHI_ENGINE_SHA256={hex}");
}

fn main() {
    embed_engine_hash();
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
