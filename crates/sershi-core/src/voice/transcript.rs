//! What the recogniser heard, and whether SERSHI should act on it.
//!
//! A transcript is **untrusted input**, exactly like typed text: an accepted
//! transcript is submitted through the ordinary command pipeline and policy.
//! This module only filters out results that are not worth submitting at all
//! (silence, recogniser hallucinations, low confidence), preferring "I
//! couldn't understand that" over running a guessed command.

use crate::service::MAX_COMMAND_CHARS;

use super::language::LanguageTag;

/// A recogniser result. Never logged or persisted by SERSHI; the accepted
/// text is shown to the user ("You said…") and then handled like typed text.
#[derive(Clone, PartialEq)]
pub struct Transcript {
    pub text: String,
    /// The language the recogniser used (detected or hinted).
    pub language: Option<LanguageTag>,
    /// Recogniser's probability that the audio held no speech (0–1).
    pub no_speech_probability: f32,
    /// Mean token probability (0–1); a rough confidence.
    pub confidence: f32,
}

// Transcripts may contain what the user said: keep them out of debug logs.
impl std::fmt::Debug for Transcript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transcript")
            .field("chars", &self.text.chars().count())
            .field("language", &self.language)
            .field("no_speech_probability", &self.no_speech_probability)
            .field("confidence", &self.confidence)
            .finish()
    }
}

/// What to do with a transcript.
#[derive(Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Submit this text through the command pipeline.
    Accept(String),
    /// Nothing was said.
    NoSpeech,
    /// Something was said, but not clearly enough to act on.
    Unclear,
}

impl std::fmt::Debug for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept(text) => write!(f, "Accept({} chars)", text.chars().count()),
            Self::NoSpeech => f.write_str("NoSpeech"),
            Self::Unclear => f.write_str("Unclear"),
        }
    }
}

/// Above this, the recogniser itself believes there was no speech.
const NO_SPEECH_THRESHOLD: f32 = 0.6;
/// Below this mean token probability, SERSHI asks again instead of acting.
const MIN_CONFIDENCE: f32 = 0.4;

/// Phrases Whisper-family models are known to produce from silence or
/// noise (subtitle credits and sign-offs from their training data). Compared
/// after normalisation; these are never acted on.
const HALLUCINATIONS: &[&str] = &[
    "thank you",
    "thanks for watching",
    "thank you for watching",
    "please subscribe",
    "you",
    "bye",
    "gracias",
    "gracias por ver",
    "gracias por ver el video",
    "subtítulos realizados por la comunidad de amara org",
    "subtítulos por la comunidad de amara org",
    "suscríbete",
    "obrigado",
    "obrigado por assistir",
    "legendas pela comunidade amara org",
    "inscreva se",
];

/// Decides whether a transcript should be submitted.
pub fn assess(transcript: &Transcript) -> Verdict {
    let text = clean(&transcript.text);
    if text.is_empty() || transcript.no_speech_probability > NO_SPEECH_THRESHOLD {
        return Verdict::NoSpeech;
    }
    if HALLUCINATIONS.contains(&normalise(&text).as_str()) {
        return Verdict::NoSpeech;
    }
    if transcript.confidence < MIN_CONFIDENCE || text.chars().count() > MAX_COMMAND_CHARS {
        return Verdict::Unclear;
    }
    Verdict::Accept(text)
}

/// Removes non-speech annotations (`[Music]`, `(risas)`, `♪`), stray quotes
/// and whitespace; keeps the words as spoken.
pub fn clean(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0u32;
    for c in raw.chars() {
        match c {
            '[' | '(' | '{' | '<' => depth += 1,
            ']' | ')' | '}' | '>' => depth = depth.saturating_sub(1),
            '♪' | '♫' | '*' => {}
            c if depth == 0 && !c.is_control() => out.push(c),
            _ => {}
        }
    }
    let words: Vec<&str> = out.split_whitespace().collect();
    words
        .join(" ")
        .trim_matches(|c: char| "\"'“”«»-–—".contains(c) || c.is_whitespace())
        .to_owned()
}

/// Lower-case letters and digits only, single-spaced (for comparisons).
fn normalise(text: &str) -> String {
    let lowered = text.to_lowercase();
    let words: Vec<&str> = lowered
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(text: &str, confidence: f32, no_speech: f32) -> Transcript {
        Transcript {
            text: text.to_owned(),
            language: LanguageTag::parse("es").ok(),
            no_speech_probability: no_speech,
            confidence,
        }
    }

    #[test]
    fn clear_speech_is_accepted_as_spoken() {
        assert_eq!(
            assess(&heard(" Abre Spotify.", 0.9, 0.01)),
            Verdict::Accept("Abre Spotify.".into())
        );
    }

    #[test]
    fn silence_and_annotations_are_no_speech() {
        for text in ["", "   ", "[Música]", "(silencio)", "♪ ♪", "[BLANK_AUDIO]"] {
            assert_eq!(
                assess(&heard(text, 0.9, 0.01)),
                Verdict::NoSpeech,
                "{text:?}"
            );
        }
        assert_eq!(assess(&heard("Abre Spotify", 0.9, 0.9)), Verdict::NoSpeech);
    }

    #[test]
    fn known_hallucinations_never_become_commands() {
        for text in [
            "Thank you.",
            "¡Gracias por ver el video!",
            "Subtítulos realizados por la comunidad de Amara.org",
            "Obrigado por assistir.",
        ] {
            assert_eq!(assess(&heard(text, 0.95, 0.0)), Verdict::NoSpeech, "{text}");
        }
    }

    #[test]
    fn low_confidence_asks_again_instead_of_guessing() {
        assert_eq!(
            assess(&heard("Cierra Spotify", 0.2, 0.05)),
            Verdict::Unclear
        );
    }

    #[test]
    fn overlong_transcripts_are_not_submitted() {
        let long = "abre ".repeat(MAX_COMMAND_CHARS);
        assert_eq!(assess(&heard(&long, 0.9, 0.0)), Verdict::Unclear);
    }

    #[test]
    fn debug_output_never_contains_the_words() {
        let t = heard("Cierra Spotify", 0.9, 0.0);
        assert!(!format!("{t:?}").contains("Spotify"));
        assert!(!format!("{:?}", assess(&t)).contains("Spotify"));
    }

    #[test]
    fn cleaning_keeps_words_and_drops_markup() {
        assert_eq!(clean("  \"Abre   Spotify\" [ruido] "), "Abre Spotify");
        assert_eq!(clean("Open (inaudible) Notepad"), "Open Notepad");
    }
}
