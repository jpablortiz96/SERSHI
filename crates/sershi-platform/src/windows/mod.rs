//! Windows-only integrations.
//!
//! This module is the single home for Win32 / WinRT calls. It compiles only
//! for `target_os = "windows"` and is checked by the `windows-latest` CI job.
//!
//! Planned residents (see `docs/WINDOWS_PLATFORM.md`):
//! - application launcher (App Paths registry + Start menu shortcuts, launched
//!   via `ShellExecuteW` with a resolved executable — never a shell string)
//! - battery status (`GetSystemPowerStatus`)
//! - credential store (Windows Credential Manager)
//! - global shortcut and tray integration (in the desktop shell)
//!
//! Every item is REQUIRES_WINDOWS_VALIDATION until verified on hardware.
//! Nothing is implemented yet: an empty boundary is better than a fake one.
