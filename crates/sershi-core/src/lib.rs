//! SERSHI core domain.
//!
//! This crate is deliberately portable: it knows nothing about Tauri, React,
//! Windows, or any AI vendor. Platform capabilities enter through the ports in
//! [`ports`]; everything else is plain, testable Rust.
//!
//! The one rule this crate exists to enforce: **nothing reaches the operating
//! system except a registered, typed [`tool::Tool`] that has passed the
//! [`policy::PolicyEngine`].** See `docs/AGENT_MODEL.md`.

pub mod activity;
pub mod apps;
pub mod assistant;
pub mod brain;
pub mod builtin;
pub mod confirmation;
pub mod executor;
pub mod ids;
pub mod intent;
pub mod ipc;
pub mod permission;
pub mod platform;
pub mod policy;
pub mod ports;
pub mod service;
pub mod shortcut;
pub mod system;
pub mod tool;
pub mod understanding;
pub mod voice;

#[cfg(all(test, feature = "ts"))]
mod contract_fixtures;
