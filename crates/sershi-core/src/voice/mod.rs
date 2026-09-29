//! Voice: push-to-talk input and spoken output, local-first.
//!
//! ```text
//! push-to-talk ─► capture (memory only) ─► endpoint ─► 16 kHz mono
//!   ─► local recognition ─► assess ─► AssistantService::submit_transcript
//!   ─► the same intent → tool → policy → confirmation pipeline as typed text
//! outcome ─► localized reply ─► local synthesis ─► playback (Speaking)
//! ```
//!
//! # Audio does not grant authority
//!
//! Voice is an input and an output modality. A transcript is untrusted text
//! submitted through the typed-command path; nothing in this module can
//! reach [`crate::service::AssistantService::decide`], learn a confirmation
//! id, or change a permission. "Yes", "sí" or "approve" spoken aloud are
//! ordinary requests — which, like any new request, cancel a pending
//! approval rather than grant it. See docs/VOICE.md and ADR 0014.
//!
//! This module is portable and pure (no devices, files or threads); the
//! Windows adapters live in `sershi-platform`, orchestration in the desktop
//! shell.

pub mod endpoint;
pub mod language;
pub mod models;
pub mod ports;
pub mod signal;
pub mod transcript;
mod types;

pub use language::{CONVERSATION_LANGUAGES, InvalidLanguageTag, LanguageTag};
pub use models::{DEFAULT_STT_MODEL, ModelTier, STT_MODELS, SttModel};
pub use types::{
    CaptureStart, LevelSource, MAX_VOICE_ID, MicrophoneAccess, MicrophoneStatus, ModelError,
    ModelInfo, ModelProgress, ModelState, VoiceFailure, VoiceLevel, VoiceSettings, VoiceStatus,
    VoiceUpdate,
};
