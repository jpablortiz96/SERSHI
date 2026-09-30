//! Voice ports. Adapters live outside the core: `sershi-platform` implements
//! them for Windows (WASAPI capture and output, whisper.cpp recognition,
//! Windows speech synthesis). The core never sees a device handle, a model
//! file or a vendor type.
//!
//! Raw audio crosses these ports in memory only. No port writes audio to
//! disk, and nothing here is ever sent to the UI except a bounded level.

use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use thiserror::Error;

use super::language::LanguageTag;
use super::latency::Acceleration;
use super::transcript::Transcript;

// ── Capture ───────────────────────────────────────────────────────────────

/// A microphone as the user may choose it in Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    /// Stable endpoint identifier (persisted as the user's choice).
    pub id: String,
    /// Name as Windows shows it.
    pub name: String,
    pub is_default: bool,
}

/// The device's native stream format (before normalisation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CaptureError {
    #[error("no microphone is connected")]
    NoDevice,
    #[error("the selected microphone is not connected")]
    DeviceNotFound,
    #[error("microphone access is turned off in Windows privacy settings")]
    PermissionDenied,
    #[error("the microphone is unavailable or was disconnected")]
    DeviceUnavailable,
    #[error("the microphone's audio format is not supported")]
    UnsupportedFormat,
    #[error("voice input is not available on this platform")]
    Unsupported,
    /// Internal detail for diagnostics; never shown to users verbatim.
    #[error("audio capture failed: {0}")]
    Failed(String),
}

/// Receives captured audio. Called from the audio thread: implementations
/// must return quickly and must not block on the UI.
pub trait CaptureSink: Send {
    /// Interleaved `f32` samples in the device's format.
    fn audio(&mut self, interleaved: &[f32]);
    /// The stream failed (e.g. the device was unplugged). No more audio
    /// follows.
    fn failed(&mut self, error: CaptureError);
}

/// A running capture. The microphone is released when [`Self::stop`] is
/// called or the handle is dropped — whichever happens first.
pub trait ActiveCapture: Send + Debug {
    fn stop(self: Box<Self>);
}

pub trait AudioCapturePort: Send + Sync + Debug {
    fn input_devices(&self) -> Result<Vec<InputDevice>, CaptureError>;
    /// Opens `device` (or the system default for `None`) and starts
    /// delivering audio to `sink`.
    fn start(
        &self,
        device: Option<&str>,
        sink: Box<dyn CaptureSink>,
    ) -> Result<(CaptureFormat, Box<dyn ActiveCapture>), CaptureError>;
}

// ── Recognition ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SttError {
    #[error("the speech model is not installed")]
    ModelMissing,
    #[error("the speech model is damaged")]
    ModelCorrupt,
    #[error("the language is not supported by the speech model")]
    UnsupportedLanguage,
    #[error("speech recognition is not available on this platform")]
    Unsupported,
    /// The recognition was cancelled (the user dismissed it); its result
    /// is discarded and never submitted.
    #[error("speech recognition was cancelled")]
    Cancelled,
    /// Internal detail for diagnostics; never shown to users verbatim.
    #[error("speech recognition failed: {0}")]
    Failed(String),
}

/// How to recognise one utterance.
#[derive(Debug, Clone, Default)]
pub struct TranscribeOptions {
    /// `None` asks the recogniser to detect the spoken language.
    pub language: Option<LanguageTag>,
    /// Short recognition context (installed application names). It only
    /// helps spelling; it never selects or authorizes anything.
    pub context: Option<String>,
    /// Set to abort the recognition as soon as possible.
    pub cancel: Option<Arc<AtomicBool>>,
}

/// Local speech-to-text. Input is 16 kHz mono `f32`
/// ([`super::signal::RECOGNITION_RATE`]).
pub trait SpeechToTextPort: Send + Sync + Debug {
    fn transcribe(
        &self,
        audio: &[f32],
        options: &TranscribeOptions,
    ) -> Result<Transcript, SttError>;
    /// Where this engine runs.
    fn acceleration(&self) -> Acceleration;
}

// ── Synthesis and playback ────────────────────────────────────────────────

/// An installed system voice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SynthesisVoice {
    /// Stable identifier (persisted as the user's choice).
    pub id: String,
    pub name: String,
    /// BCP-47 tag as the system reports it (e.g. `es-ES`).
    pub language: String,
}

/// Synthesized speech, mono PCM, in memory only.
#[derive(Clone, PartialEq)]
pub struct SpeechAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Debug for SpeechAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpeechAudio")
            .field("samples", &self.samples.len())
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

/// Which voice to speak with. An explicit voice wins; otherwise the first
/// installed voice for `language`; otherwise the system default voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceChoice<'a> {
    pub voice_id: Option<&'a str>,
    pub language: Option<&'a LanguageTag>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TtsError {
    #[error("no speech voice is installed")]
    NoVoice,
    #[error("speech synthesis is not available on this platform")]
    Unsupported,
    /// Internal detail for diagnostics; never shown to users verbatim.
    #[error("speech synthesis failed: {0}")]
    Failed(String),
}

pub trait SpeechSynthesisPort: Send + Sync + Debug {
    fn voices(&self) -> Result<Vec<SynthesisVoice>, TtsError>;
    fn synthesize(&self, text: &str, voice: VoiceChoice<'_>) -> Result<SpeechAudio, TtsError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlaybackError {
    #[error("no audio output device is available")]
    NoOutput,
    #[error("audio playback is not available on this platform")]
    Unsupported,
    /// Internal detail for diagnostics; never shown to users verbatim.
    #[error("audio playback failed: {0}")]
    Failed(String),
}

/// How a playback ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackEnd {
    Completed,
    Stopped,
    Failed,
}

/// Receives playback progress (from the audio thread; return quickly).
pub trait PlaybackSink: Send {
    /// Mono samples just handed to the device (for the output level).
    fn played(&mut self, mono: &[f32]);
    /// Called exactly once.
    fn finished(&mut self, end: PlaybackEnd);
}

/// A running playback; stopping is idempotent.
pub trait ActivePlayback: Send + Debug {
    fn stop(self: Box<Self>);
}

pub trait AudioOutputPort: Send + Sync + Debug {
    fn play(
        &self,
        audio: SpeechAudio,
        sink: Box<dyn PlaybackSink>,
    ) -> Result<Box<dyn ActivePlayback>, PlaybackError>;
}

// ── Wake word (future) ────────────────────────────────────────────────────

/// **Not implemented; nothing calls it.** The shape of a future local
/// wake-word detector (Prompt 3B, only after Gate 3A passes).
///
/// Target behaviour: when the user explicitly enables a wake word, a local
/// detector consumes microphone audio *inside SERSHI* and reports a
/// detection; SERSHI then shows the same unmistakable Listening indicator
/// and enters the push-to-talk pipeline. A detection is an invocation, never
/// authorization: wake word ≠ identity ≠ approval. Until then, the
/// microphone opens only on an explicit push-to-talk.
pub trait WakeWordPort: Send + Sync + Debug {
    /// Feeds 16 kHz mono audio; returns true when the phrase was detected.
    fn detect(&mut self, audio: &[f32]) -> bool;
}
