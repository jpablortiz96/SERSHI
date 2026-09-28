//! Application control: discovery, resolution, and the open/close tools.
//! See docs/APPLICATIONS.md.

pub mod catalog;
pub mod manager;
pub mod model;
pub mod normalize;
pub mod tools;

pub use catalog::{ApplicationCatalog, MatchKind, Resolution};
pub use manager::{ApplicationManager, CatalogState, CatalogStatus};
pub use model::{
    AppSource, ApplicationDescriptor, ApplicationSummary, CloseSupport, LaunchTarget, slug,
};
pub use tools::{ApplicationResult, LaunchFailure};
