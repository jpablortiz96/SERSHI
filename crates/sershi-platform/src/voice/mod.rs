//! Voice adapters for the ports in `sershi_core::voice::ports`.
//!
//! Windows has the real implementations (`crate::windows::voice`); other
//! platforms report voice as unsupported rather than pretending. The model
//! store and WAV decoding are portable and tested everywhere.

pub mod model_store;
pub mod wav;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use sershi_core::voice::latency::{Acceleration, AcceleratorProbe, choose_acceleration};
use sershi_core::voice::ports::{
    AudioCapturePort, AudioOutputPort, SpeechSynthesisPort, SpeechToTextPort, SttError,
};
use sershi_core::voice::{MicrophoneAccess, ModelError, SttModel};

pub use model_store::ModelStore;

/// The device-facing voice adapters for this platform.
#[derive(Debug, Clone)]
pub struct VoicePlatform {
    /// Voice is implemented on this platform.
    pub supported: bool,
    pub capture: Arc<dyn AudioCapturePort>,
    pub output: Arc<dyn AudioOutputPort>,
    pub synthesis: Arc<dyn SpeechSynthesisPort>,
}

pub fn voice_platform() -> VoicePlatform {
    #[cfg(windows)]
    {
        use crate::windows::voice::{WasapiCapture, WasapiOutput, WindowsSpeech};
        VoicePlatform {
            supported: true,
            capture: Arc::new(WasapiCapture),
            output: Arc::new(WasapiOutput),
            synthesis: Arc::new(WindowsSpeech),
        }
    }
    #[cfg(not(windows))]
    {
        VoicePlatform {
            supported: false,
            capture: Arc::new(unsupported::Unsupported),
            output: Arc::new(unsupported::Unsupported),
            synthesis: Arc::new(unsupported::Unsupported),
        }
    }
}

/// What GPU acceleration is available for speech recognition here.
pub fn accelerator_probe() -> AcceleratorProbe {
    #[cfg(windows)]
    {
        crate::windows::voice::gpu::probe()
    }
    #[cfg(not(windows))]
    {
        AcceleratorProbe {
            compiled: false,
            runtime: false,
            devices: Vec::new(),
        }
    }
}

/// The best supported accelerator (and its name); the CPU otherwise.
pub fn speech_acceleration() -> (Acceleration, Option<String>) {
    choose_acceleration(&accelerator_probe())
}

/// Loads a verified model into the local recogniser on `acceleration`
/// (falling back to the CPU). Expensive (hundreds of MB, one to two
/// seconds); callers load lazily and keep the result warm.
pub fn load_recognizer(
    model: &Path,
    acceleration: Acceleration,
) -> Result<Arc<dyn SpeechToTextPort>, SttError> {
    #[cfg(windows)]
    {
        Ok(Arc::new(crate::windows::voice::WhisperRecognizer::load(
            model,
            acceleration,
        )?))
    }
    #[cfg(not(windows))]
    {
        let _ = (model, acceleration);
        Err(SttError::Unsupported)
    }
}

/// Whether Windows privacy settings let SERSHI use the microphone.
pub fn microphone_access() -> MicrophoneAccess {
    #[cfg(windows)]
    {
        crate::windows::voice::microphone_access()
    }
    #[cfg(not(windows))]
    {
        MicrophoneAccess::Unknown
    }
}

/// Downloads `model` from its fixed catalog URL into `store`, verifying
/// size and SHA-256 before it is moved into place.
pub fn download_model(
    store: &ModelStore,
    model: &SttModel,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<(), ModelError> {
    #[cfg(windows)]
    {
        let mut body =
            crate::windows::voice::HttpsGet::open(&model.url()).map_err(|_| ModelError::Network)?;
        store.install(model, &mut body, progress, cancel)
    }
    #[cfg(not(windows))]
    {
        let _ = (store, model, progress, cancel);
        Err(ModelError::Network)
    }
}

#[cfg(not(windows))]
mod unsupported {
    use sershi_core::voice::ports::{
        ActiveCapture, ActivePlayback, AudioCapturePort, AudioOutputPort, CaptureError,
        CaptureFormat, CaptureSink, InputDevice, PlaybackError, PlaybackSink, SpeechAudio,
        SpeechSynthesisPort, SynthesisVoice, TtsError, VoiceChoice,
    };

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Unsupported;

    impl AudioCapturePort for Unsupported {
        fn input_devices(&self) -> Result<Vec<InputDevice>, CaptureError> {
            Err(CaptureError::Unsupported)
        }
        fn start(
            &self,
            _: Option<&str>,
            _: Box<dyn CaptureSink>,
        ) -> Result<(CaptureFormat, Box<dyn ActiveCapture>), CaptureError> {
            Err(CaptureError::Unsupported)
        }
    }

    impl AudioOutputPort for Unsupported {
        fn play(
            &self,
            _: SpeechAudio,
            _: Box<dyn PlaybackSink>,
        ) -> Result<Box<dyn ActivePlayback>, PlaybackError> {
            Err(PlaybackError::Unsupported)
        }
    }

    impl SpeechSynthesisPort for Unsupported {
        fn voices(&self) -> Result<Vec<SynthesisVoice>, TtsError> {
            Ok(Vec::new())
        }
        fn synthesize(&self, _: &str, _: VoiceChoice<'_>) -> Result<SpeechAudio, TtsError> {
            Err(TtsError::Unsupported)
        }
    }
}
