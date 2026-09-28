//! Operating-system adapters for the ports defined in `sershi-core`.
//!
//! Layout:
//! - [`system_info`] — portable adapter (works on Windows, Linux and macOS).
//! - [`capabilities`] — honest per-platform capability report.
//! - `windows` — Windows-only integrations, compiled only for
//!   `target_os = "windows"`. Nothing outside this module may call Win32.
//!
//! Every Windows behaviour is `REQUIRES_WINDOWS_VALIDATION` until it has been
//! exercised on a physical Windows machine; see `docs/WINDOWS_PLATFORM.md`.

pub mod capabilities;
pub mod system_info;

#[cfg(windows)]
mod windows;

pub use capabilities::capabilities;
pub use system_info::SysinfoSystemInfo;
