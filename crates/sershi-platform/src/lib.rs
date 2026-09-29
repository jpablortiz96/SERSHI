//! Operating-system adapters for the ports defined in `sershi-core`.
//!
//! Layout:
//! - [`system_info`] — portable adapter (works on Windows, Linux and macOS).
//! - [`capabilities`] — honest per-platform capability report.
//! - [`voice`] — microphone, speech recognition and synthesis adapters
//!   (Windows), plus the portable model store.
//! - `windows` — Windows-only integrations, compiled only for
//!   `target_os = "windows"`. Nothing outside this module may call Win32.
//!
//! Every Windows behaviour is `REQUIRES_WINDOWS_VALIDATION` until it has been
//! exercised on a physical Windows machine; see `docs/WINDOWS_PLATFORM.md`.

pub mod capabilities;
pub mod system_info;
pub mod voice;

#[cfg(windows)]
mod windows;

use std::sync::Arc;

use sershi_core::ports::ApplicationPlatform;

pub use capabilities::capabilities;
pub use system_info::SysinfoSystemInfo;

/// The application-control adapter for this platform.
pub fn application_platform() -> Arc<dyn ApplicationPlatform> {
    #[cfg(windows)]
    {
        Arc::new(windows::WindowsApplications)
    }
    #[cfg(not(windows))]
    {
        Arc::new(sershi_core::ports::UnsupportedApplications)
    }
}
