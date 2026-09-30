//! GPU acceleration for speech recognition (Vulkan through ggml).
//!
//! SERSHI links whisper.cpp's Vulkan backend with `vulkan-1.dll`
//! delay-loaded (see `build.rs`). The Vulkan loader ships with every modern
//! NVIDIA, AMD and Intel driver; on a machine without it (e.g. a VM with no
//! GPU driver) nothing may call into whisper.cpp at all, because its backend
//! registry enumerates Vulkan devices on first use. [`vulkan_runtime`]
//! therefore gates every recogniser load.
//!
//! The loader is loaded from System32 only (never from the application
//! directory or the working directory), and kept loaded, so the delay-load
//! helper binds to that copy — a planted `vulkan-1.dll` next to SERSHI is
//! never used.

use std::sync::OnceLock;

use sershi_core::voice::latency::AcceleratorProbe;

/// Whether this build contains the GPU backend.
pub const COMPILED: bool = cfg!(feature = "vulkan");

/// Whether the Vulkan loader is available from System32 (loaded once and
/// kept loaded).
pub fn vulkan_runtime() -> bool {
    static PRESENT: OnceLock<bool> = OnceLock::new();
    *PRESENT.get_or_init(|| {
        use windows::Win32::System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW};
        use windows::core::w;
        // SAFETY: loads a system DLL by name from System32 only; the handle
        // is intentionally never freed.
        unsafe { LoadLibraryExW(w!("vulkan-1.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.is_ok()
    })
}

/// Whether whisper.cpp may be called in this process at all.
pub fn backend_usable() -> bool {
    !COMPILED || vulkan_runtime()
}

/// What the backend can accelerate on this machine.
pub fn probe() -> AcceleratorProbe {
    let runtime = COMPILED && vulkan_runtime();
    AcceleratorProbe {
        compiled: COMPILED,
        runtime,
        devices: if runtime { devices() } else { Vec::new() },
    }
}

/// ggml's registered devices as `(kind, description)`.
fn devices() -> Vec<(String, String)> {
    use whisper_rs::whisper_rs_sys as sys;
    let mut out = Vec::new();
    // SAFETY: ggml's device registry is initialized lazily and is
    // thread-safe; indices are bounded by `ggml_backend_dev_count`, and the
    // returned C strings are owned by ggml for the process lifetime.
    unsafe {
        let count = sys::ggml_backend_dev_count();
        for i in 0..count {
            let device = sys::ggml_backend_dev_get(i);
            if device.is_null() {
                continue;
            }
            let kind = match sys::ggml_backend_dev_type(device) {
                sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_CPU => "cpu",
                sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU => "gpu",
                _ => "other",
            };
            let description = sys::ggml_backend_dev_description(device);
            let name = if description.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(description)
                    .to_string_lossy()
                    .trim()
                    .to_owned()
            };
            out.push((kind.to_owned(), name));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use sershi_core::voice::latency::{Acceleration, choose_acceleration};

    use super::*;

    #[test]
    fn without_the_vulkan_loader_nothing_reports_a_gpu() {
        let probe = probe();
        assert_eq!(probe.compiled, COMPILED);
        if !probe.runtime {
            assert!(probe.devices.is_empty());
            assert_eq!(choose_acceleration(&probe).0, Acceleration::Cpu);
        }
        // whisper.cpp may be called exactly when the loader (if needed) is
        // present; the check is stable across calls.
        assert_eq!(backend_usable(), !COMPILED || vulkan_runtime());
        assert_eq!(vulkan_runtime(), vulkan_runtime());
    }

    #[test]
    fn a_reported_accelerator_is_a_real_gpu_device() {
        let probe = probe();
        if let (Acceleration::Vulkan, Some(name)) = choose_acceleration(&probe) {
            assert!(probe.runtime && probe.compiled);
            assert!(probe.devices.iter().any(|(k, n)| k == "gpu" && *n == name));
        }
    }
}
