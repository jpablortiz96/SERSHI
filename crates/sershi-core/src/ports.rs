//! Ports: the interfaces through which the core reaches the outside world.
//!
//! Adapters live outside this crate (`sershi-platform` for OS access, future
//! provider crates for AI vendors). Ports are added only when a real boundary
//! exists — see ADR 0002.

use thiserror::Error;

use crate::apps::{ApplicationDescriptor, CloseSupport, LaunchTarget};
use crate::system::SystemSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortError {
    #[error("not available on this platform: {0}")]
    Unsupported(String),
    #[error("platform call failed: {0}")]
    Platform(String),
}

/// Read-only access to machine information.
pub trait SystemInfoProvider: Send + Sync + std::fmt::Debug {
    fn snapshot(&self) -> Result<SystemSnapshot, PortError>;
}

/// Discovers, starts and closes applications. Implemented natively for
/// Windows in `sershi-platform`; other platforms report themselves as
/// unsupported. The core decides *which* application; the port only executes
/// an already-resolved [`LaunchTarget`] / [`CloseSupport`].
pub trait ApplicationPlatform: Send + Sync + std::fmt::Debug {
    fn is_supported(&self) -> bool;
    fn discover(&self) -> Result<Vec<ApplicationDescriptor>, PortError>;
    fn launch(&self, target: &LaunchTarget) -> Result<(), ApplicationError>;
    /// Whether the application is running with windows SERSHI could close.
    fn running_state(&self, close: &CloseSupport) -> Result<RunningState, ApplicationError>;
    /// Politely asks the application's top-level windows to close
    /// (`WM_CLOSE`). Never terminates processes.
    fn close(&self, close: &CloseSupport) -> Result<RunningState, ApplicationError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunningState {
    /// Running, with this many closable top-level windows.
    Running {
        windows: u32,
    },
    NotRunning,
    /// Running, but without a window SERSHI can safely ask to close (e.g.
    /// minimized to the tray).
    NoClosableWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplicationError {
    #[error("the application's launch target no longer exists")]
    TargetMissing,
    #[error("access denied")]
    AccessDenied,
    #[error("the application requires administrator rights")]
    ElevationRequired,
    #[error("this kind of application is not supported")]
    Unsupported,
    #[error("the operation timed out")]
    TimedOut,
    /// Internal detail for diagnostics; never shown to users verbatim.
    #[error("platform call failed: {0}")]
    Failed(String),
    #[error("application control is not available on this platform")]
    PlatformUnavailable,
}

/// The adapter for platforms without application control.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedApplications;

impl ApplicationPlatform for UnsupportedApplications {
    fn is_supported(&self) -> bool {
        false
    }
    fn discover(&self) -> Result<Vec<ApplicationDescriptor>, PortError> {
        Err(PortError::Unsupported("application discovery".to_owned()))
    }
    fn launch(&self, _: &LaunchTarget) -> Result<(), ApplicationError> {
        Err(ApplicationError::PlatformUnavailable)
    }
    fn running_state(&self, _: &CloseSupport) -> Result<RunningState, ApplicationError> {
        Err(ApplicationError::PlatformUnavailable)
    }
    fn close(&self, _: &CloseSupport) -> Result<RunningState, ApplicationError> {
        Err(ApplicationError::PlatformUnavailable)
    }
}
