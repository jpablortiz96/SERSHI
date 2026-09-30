//! Windows-only integrations.
//!
//! This module is the single home for Win32 / COM calls and the only place in
//! SERSHI allowed to use `unsafe` (FFI). It compiles only for
//! `target_os = "windows"`; the cloud checks it with
//! `cargo check --target x86_64-pc-windows-msvc` and CI builds and tests it on
//! `windows-latest`.
//!
//! Everything here is REQUIRES_WINDOWS_VALIDATION until exercised on a
//! physical machine (docs/WINDOWS_PLATFORM.md).
//!
//! Launching never goes through a shell interpreter (`cmd`, PowerShell) and
//! never builds a command line from user or model text: targets come from
//! discovery only (see docs/APPLICATIONS.md).

#![allow(unsafe_code)]

mod app_paths;
mod close;
pub mod job;
mod known;
mod launch;
mod packaged;
mod start_menu;
mod util;
pub mod voice;

use std::time::Duration;

use sershi_core::apps::{ApplicationDescriptor, CloseSupport, LaunchTarget};
use sershi_core::ports::{ApplicationError, ApplicationPlatform, PortError, RunningState};

/// Upper bounds for native operations; a hung call returns an error instead
/// of blocking SERSHI.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(20);
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Native application discovery, launch and graceful close.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsApplications;

impl ApplicationPlatform for WindowsApplications {
    fn is_supported(&self) -> bool {
        true
    }

    fn discover(&self) -> Result<Vec<ApplicationDescriptor>, PortError> {
        util::with_timeout(DISCOVERY_TIMEOUT, || {
            let _com = util::ComApartment::init();
            let mut apps = known::builtins();
            // Each source is best-effort: one failing source must not hide
            // the others.
            packaged::discover(&mut apps);
            start_menu::discover(&mut apps);
            app_paths::discover(&mut apps);
            apps
        })
        .ok_or_else(|| PortError::Platform("application discovery timed out".to_owned()))
    }

    fn launch(&self, target: &LaunchTarget) -> Result<(), ApplicationError> {
        let target = target.clone();
        util::with_timeout(LAUNCH_TIMEOUT, move || {
            let _com = util::ComApartment::init();
            launch::launch(&target)
        })
        .unwrap_or(Err(ApplicationError::TimedOut))
    }

    fn running_state(&self, close: &CloseSupport) -> Result<RunningState, ApplicationError> {
        let close = close.clone();
        util::with_timeout(CLOSE_TIMEOUT, move || close::state(&close, false))
            .unwrap_or(Err(ApplicationError::TimedOut))
    }

    fn close(&self, close: &CloseSupport) -> Result<RunningState, ApplicationError> {
        let close = close.clone();
        util::with_timeout(CLOSE_TIMEOUT, move || close::state(&close, true))
            .unwrap_or(Err(ApplicationError::TimedOut))
    }
}
