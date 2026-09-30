//! GPU builds delay-load the Vulkan loader, so the engine starts on
//! machines without a GPU driver and reports that instead of crashing
//! (`src/engine.rs` loads it from System32 first).

fn main() {
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if windows && std::env::var_os("CARGO_FEATURE_VULKAN").is_some() {
        println!("cargo:rustc-link-arg=/DELAYLOAD:vulkan-1.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }
}
