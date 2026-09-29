//! Local speech recognition with whisper.cpp (via `whisper-rs`), in process.
//!
//! Audio stays in memory: the 16 kHz utterance is handed to whisper.cpp
//! directly, never written to a file or passed to another process. The
//! model is loaded lazily on first use and kept in memory while voice is in
//! use (see docs/VOICE.md for memory figures).

use std::path::Path;
use std::sync::Once;

use sershi_core::voice::LanguageTag;
use sershi_core::voice::ports::{SpeechToTextPort, SttError};
use sershi_core::voice::transcript::Transcript;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Threads for inference. More than 8 gave no benefit on the reference
/// machine (hybrid P/E cores) and would starve the UI.
const MAX_THREADS: usize = 8;

pub struct WhisperRecognizer {
    context: WhisperContext,
    threads: i32,
}

impl std::fmt::Debug for WhisperRecognizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhisperRecognizer")
            .field("threads", &self.threads)
            .finish_non_exhaustive()
    }
}

/// whisper.cpp and ggml print diagnostics to stderr by default; routed to
/// nowhere so nothing about an utterance can reach a console or log file.
fn silence_native_logs() {
    static ONCE: Once = Once::new();
    ONCE.call_once(whisper_rs::install_logging_hooks);
}

impl WhisperRecognizer {
    /// Loads a model file that has already been verified by the model store.
    pub fn load(model: &Path) -> Result<Self, SttError> {
        silence_native_logs();
        if !model.is_file() {
            return Err(SttError::ModelMissing);
        }
        let mut params = WhisperContextParameters::default();
        // CPU only: predictable on every machine (GPU backends are a later,
        // opt-in optimisation).
        params.use_gpu(false);
        let context =
            WhisperContext::new_with_params(model, params).map_err(|_| SttError::ModelCorrupt)?;
        let threads = std::thread::available_parallelism()
            .map_or(4, usize::from)
            .clamp(1, MAX_THREADS);
        Ok(Self {
            context,
            threads: i32::try_from(threads).unwrap_or(4),
        })
    }
}

impl SpeechToTextPort for WhisperRecognizer {
    fn transcribe(
        &self,
        audio: &[f32],
        language: Option<&LanguageTag>,
    ) -> Result<Transcript, SttError> {
        let hint = match language {
            Some(tag) => {
                let code = tag.primary();
                if whisper_rs::get_lang_id(code).is_none() {
                    return Err(SttError::UnsupportedLanguage);
                }
                code.to_owned()
            }
            None => "auto".to_owned(),
        };
        let mut state = self
            .context
            .create_state()
            .map_err(|e| SttError::Failed(e.to_string()))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&hint));
        params.set_n_threads(self.threads);
        params.set_translate(false);
        // One short utterance: no carried context, one segment, no timing.
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_no_timestamps(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_temperature(0.0);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);

        state
            .full(params, audio)
            .map_err(|e| SttError::Failed(e.to_string()))?;

        let eot = self.context.token_eot();
        let mut text = String::new();
        let mut no_speech: f32 = 0.0;
        let (mut prob_sum, mut prob_n) = (0.0f32, 0u32);
        for segment in state.as_iter() {
            if let Ok(part) = segment.to_str_lossy() {
                text.push_str(&part);
            }
            no_speech = no_speech.max(segment.no_speech_probability());
            for i in 0..segment.n_tokens() {
                if let Some(token) = segment.get_token(i)
                    && token.token_id() < eot
                {
                    prob_sum += token.token_probability();
                    prob_n += 1;
                }
            }
        }
        let detected = whisper_rs::get_lang_str(state.full_lang_id_from_state())
            .and_then(|code| LanguageTag::parse(code).ok());
        Ok(Transcript {
            text,
            language: detected.or_else(|| language.cloned()),
            no_speech_probability: no_speech,
            confidence: if prob_n == 0 {
                0.0
            } else {
                prob_sum / prob_n as f32
            },
        })
    }
}
