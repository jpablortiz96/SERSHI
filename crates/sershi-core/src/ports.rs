//! Ports: the interfaces through which the core reaches the outside world.
//!
//! Adapters live outside this crate (`sershi-platform` for OS access, future
//! provider crates for AI vendors). Ports are added only when a real boundary
//! exists — see ADR 0002.

use thiserror::Error;

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
