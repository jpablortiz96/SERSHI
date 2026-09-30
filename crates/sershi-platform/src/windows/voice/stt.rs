//! Local speech recognition with whisper.cpp (via `whisper-rs`), in process.
//!
//! Audio stays in memory: the 16 kHz utterance is handed to whisper.cpp
//! directly, never written to a file or passed to another process.
//!
//! Gate 3B: the model runs on the GPU through Vulkan when one is available
//! (measured on an RTX 3050 Laptop: Whisper Small ~0.3 s instead of ~5–8 s
//! on the CPU), with the CPU as the fallback — including when a GPU load
//! fails (e.g. not enough video memory). A GPU engine is warmed up once
//! after loading so shader compilation never lands on a user's command.

use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};

/// whisper.cpp's abort callback: true once the command was cancelled.
///
/// # Safety
/// `data` must point to a live `AtomicBool` (see `transcribe`).
unsafe extern "C" fn abort_when_set(data: *mut c_void) -> bool {
    if data.is_null() {
        return false;
    }
    // SAFETY: guaranteed by the caller (a live `Arc<AtomicBool>`).
    unsafe { &*data.cast::<AtomicBool>() }.load(Ordering::Relaxed)
}

use sershi_core::voice::LanguageTag;
use sershi_core::voice::latency::{Acceleration, decode_plan, sanitize_context};
use sershi_core::voice::ports::{SpeechToTextPort, SttError, TranscribeOptions};
use sershi_core::voice::signal::RECOGNITION_RATE;
use sershi_core::voice::transcript::Transcript;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::gpu;

pub struct WhisperRecognizer {
    context: WhisperContext,
    acceleration: Acceleration,
}

impl std::fmt::Debug for WhisperRecognizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhisperRecognizer")
            .field("acceleration", &self.acceleration)
            .finish_non_exhaustive()
    }
}

/// whisper.cpp and ggml print diagnostics to stderr by default; routed to
/// nowhere so nothing about an utterance can reach a console or log file.
fn silence_native_logs() {
    static ONCE: Once = Once::new();
    ONCE.call_once(whisper_rs::install_logging_hooks);
}

fn context(model: &Path, gpu: bool) -> Result<WhisperContext, SttError> {
    let mut params = WhisperContextParameters::default();
    params.use_gpu(gpu);
    WhisperContext::new_with_params(model, params).map_err(|_| SttError::ModelCorrupt)
}

impl WhisperRecognizer {
    /// Loads a model that the model store has already verified, on the
    /// requested accelerator, falling back to the CPU.
    pub fn load(model: &Path, acceleration: Acceleration) -> Result<Self, SttError> {
        if !gpu::backend_usable() {
            // A GPU-enabled build on a machine without the Vulkan loader:
            // calling whisper.cpp would fail to bind. Never crash; report.
            return Err(SttError::Unsupported);
        }
        silence_native_logs();
        if !model.is_file() {
            return Err(SttError::ModelMissing);
        }
        let engine = match acceleration {
            Acceleration::Vulkan => match context(model, true) {
                Ok(context) => Self {
                    context,
                    acceleration: Acceleration::Vulkan,
                },
                Err(_) => Self {
                    context: context(model, false)?,
                    acceleration: Acceleration::Cpu,
                },
            },
            Acceleration::Cpu => Self {
                context: context(model, false)?,
                acceleration: Acceleration::Cpu,
            },
        };
        if engine.acceleration == Acceleration::Vulkan {
            engine.warm_up();
        }
        Ok(engine)
    }

    /// Compiles the GPU pipelines every real command uses, so none of them
    /// is compiled on a user's command. Measured on an RTX 3050 Laptop:
    /// ~25 s the first time a program runs on a machine (the driver then
    /// caches them), and the first Automatic or hinted decode compiled
    /// further shapes — so warm-up runs all three paths: a fixed language,
    /// language detection, and a recognition context. Silence only: nothing
    /// is recognised, submitted or kept.
    fn warm_up(&self) {
        let silence = vec![0.0f32; RECOGNITION_RATE as usize];
        for options in warm_up_paths() {
            let _ = self.transcribe(&silence, &options);
        }
    }
}

/// The decode paths a warm-up exercises (see `WhisperRecognizer::warm_up`).
pub fn warm_up_paths() -> [TranscribeOptions; 3] {
    let english = LanguageTag::parse("en").ok();
    [
        TranscribeOptions {
            language: english.clone(),
            ..TranscribeOptions::default()
        },
        TranscribeOptions::default(),
        TranscribeOptions {
            language: english,
            context: Some("Chrome, Outlook, Notepad.".to_owned()),
            cancel: None,
        },
    ]
}

impl SpeechToTextPort for WhisperRecognizer {
    fn transcribe(
        &self,
        audio: &[f32],
        options: &TranscribeOptions,
    ) -> Result<Transcript, SttError> {
        let plan = decode_plan(options.language.as_ref(), self.acceleration);
        let hint = match plan.language.as_deref() {
            Some(code) => {
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
        params.set_n_threads(i32::from(plan.threads));
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
        let context = options
            .context
            .as_deref()
            .map(sanitize_context)
            .filter(|c| !c.is_empty());
        if let Some(context) = context.as_deref() {
            params.set_initial_prompt(context);
        }
        // Abort as soon as the cancel flag is set. whisper-rs's
        // `set_abort_callback_safe` is not used: its trampoline reads the
        // boxed closure as the wrong type and returns arbitrary values,
        // which aborted about half of all decodes (error -6, measured).
        // `flag` is an `Arc` clone held until `full` returns, so the pointer
        // stays valid for every callback.
        let flag = options.cancel.clone();
        if let Some(flag) = flag.as_ref() {
            // SAFETY: `abort_when_set` only reads the `AtomicBool` behind
            // the pointer, which `flag` keeps alive for the whole decode.
            unsafe {
                params.set_abort_callback(Some(abort_when_set));
                params.set_abort_callback_user_data(Arc::as_ptr(flag).cast_mut().cast());
            }
        }

        let result = state.full(params, audio);
        drop(flag);
        if options
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
        {
            return Err(SttError::Cancelled);
        }
        result.map_err(|e| SttError::Failed(e.to_string()))?;

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
            language: detected.or_else(|| options.language.clone()),
            no_speech_probability: no_speech,
            confidence: if prob_n == 0 {
                0.0
            } else {
                prob_sum / prob_n as f32
            },
        })
    }

    fn acceleration(&self) -> Acceleration {
        self.acceleration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_up_covers_every_decode_path_a_command_uses() {
        let paths = warm_up_paths();
        // A fixed language, language detection and a recognition context.
        assert!(
            paths
                .iter()
                .any(|p| p.language.is_some() && p.context.is_none())
        );
        assert!(paths.iter().any(|p| p.language.is_none()));
        assert!(paths.iter().any(|p| p.context.is_some()));
        // Warm-up can never be cancelled into a half state or run a command:
        // it only decodes silence and discards the result.
        assert!(paths.iter().all(|p| p.cancel.is_none()));
    }
}
