//! Windows voice adapters: WASAPI capture and playback (`cpal`), local
//! whisper.cpp recognition, Windows speech synthesis, WinHTTP model
//! download and the microphone privacy check.
//!
//! REQUIRES_WINDOWS_VALIDATION until Gate 3A passes (docs/VOICE.md).

mod capture;
mod consent;
pub mod gpu;
mod http;
mod output;
mod stt;
mod tts;

pub use capture::WasapiCapture;
pub use consent::microphone_access;
pub use http::HttpsGet;
pub use output::WasapiOutput;
pub use stt::WhisperRecognizer;
pub use tts::WindowsSpeech;
