//! Local speech synthesis with Windows' built-in voices
//! (`Windows.Media.SpeechSynthesis`, the OneCore voices installed with
//! Windows language packs).
//!
//! Typed and local: the text is passed to the synthesizer as plain text (not
//! SSML), no shell or process is involved, no network is used, and the audio
//! is returned in memory for SERSHI's own playback.

use sershi_core::voice::LanguageTag;
use sershi_core::voice::ports::{
    SpeechAudio, SpeechSynthesisPort, SynthesisVoice, TtsError, VoiceChoice,
};
use windows::Media::SpeechSynthesis::{SpeechSynthesizer, VoiceInformation};
use windows::Storage::Streams::DataReader;
use windows::core::HSTRING;

use crate::voice::wav;

/// Longest text SERSHI will synthesize in one reply.
pub const MAX_SPEECH_CHARS: usize = 600;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsSpeech;

fn failed(e: &windows::core::Error) -> TtsError {
    TtsError::Failed(format!("{:#010x}", e.code().0))
}

fn describe(voice: &VoiceInformation) -> Option<SynthesisVoice> {
    Some(SynthesisVoice {
        id: voice.Id().ok()?.to_string(),
        name: voice.DisplayName().ok()?.to_string(),
        language: voice.Language().ok()?.to_string(),
    })
}

fn installed() -> Result<Vec<VoiceInformation>, TtsError> {
    let all = SpeechSynthesizer::AllVoices().map_err(|e| failed(&e))?;
    Ok(all.into_iter().collect())
}

/// Picks a voice: explicit id, else one for the language (preferring the
/// system default when it matches), else the system default.
fn choose(choice: VoiceChoice<'_>) -> Result<Option<VoiceInformation>, TtsError> {
    let voices = installed()?;
    if voices.is_empty() {
        return Err(TtsError::NoVoice);
    }
    if let Some(id) = choice.voice_id
        && let Some(v) = voices.iter().find(|v| v.Id().is_ok_and(|vid| vid == id))
    {
        return Ok(Some(v.clone()));
    }
    let default = SpeechSynthesizer::DefaultVoice().ok();
    if let Some(language) = choice.language {
        let matches = |v: &VoiceInformation| {
            v.Language()
                .ok()
                .and_then(|l| LanguageTag::parse(&l.to_string()).ok())
                .is_some_and(|l| l.same_language(language))
        };
        if let Some(d) = default.as_ref().filter(|d| matches(d)) {
            return Ok(Some(d.clone()));
        }
        if let Some(v) = voices.iter().find(|v| matches(v)) {
            return Ok(Some(v.clone()));
        }
    }
    Ok(default)
}

impl SpeechSynthesisPort for WindowsSpeech {
    fn voices(&self) -> Result<Vec<SynthesisVoice>, TtsError> {
        Ok(installed()?.iter().filter_map(describe).collect())
    }

    fn synthesize(&self, text: &str, voice: VoiceChoice<'_>) -> Result<SpeechAudio, TtsError> {
        let text: String = text.chars().take(MAX_SPEECH_CHARS).collect();
        let synthesizer = SpeechSynthesizer::new().map_err(|e| failed(&e))?;
        if let Some(chosen) = choose(voice)? {
            synthesizer.SetVoice(&chosen).map_err(|e| failed(&e))?;
        }
        let stream = synthesizer
            .SynthesizeTextToStreamAsync(&HSTRING::from(text.as_str()))
            .and_then(|op| op.join())
            .map_err(|e| failed(&e))?;
        let size = u32::try_from(stream.Size().map_err(|e| failed(&e))?)
            .map_err(|_| TtsError::Failed("speech too long".to_owned()))?;
        let reader =
            DataReader::CreateDataReader(&stream.GetInputStreamAt(0).map_err(|e| failed(&e))?)
                .map_err(|e| failed(&e))?;
        reader
            .LoadAsync(size)
            .and_then(|op| op.join())
            .map_err(|e| failed(&e))?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes).map_err(|e| failed(&e))?;
        wav::decode(&bytes).map_err(|e| TtsError::Failed(format!("{e:?}")))
    }
}
