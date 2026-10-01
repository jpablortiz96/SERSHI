//! Language stabilization for Automatic speech recognition (Gate 3C.1).
//!
//! Whisper detects the spoken language from the audio itself. On a short
//! command it sometimes picks an unrelated language: Spanish heard as
//! Icelandic ("Ári Óðluk") or written in Cyrillic ("Пон Мекром"). Physical
//! testing showed that its language confidence is **no guide**: Whisper
//! Large v3 Turbo was 97–99 % sure that short, real Spanish speech was
//! Icelandic, with Spanish at 0.00. So nothing here trusts it.
//!
//! Three languages are kept apart:
//!
//! - the **interface language** (what SERSHI writes and speaks; unaffected
//!   by anything here),
//! - the **conversation language** preference (Automatic, or a fixed
//!   language, in which case no detection and no stabilization happens),
//! - the **detected language** of one utterance (only a recognition fact).
//!
//! Policy ([`plan_retry`]): one second pass, at most, and only when all
//! hold —
//!
//! 1. the conversation language is Automatic;
//! 2. the first transcript is **not** already understood by SERSHI's
//!    deterministic tiers (if it is, nothing is retried and nothing is
//!    paid: "Open Chrome" heard as Icelandic still opens Chrome);
//! 3. the detected language is outside SERSHI's languages (Spanish,
//!    English, Portuguese — a prior, not a whitelist);
//! 4. the utterance is short (a command, not a conversation in another
//!    language);
//! 5. the conversation itself names a language: the last *confidently*
//!    detected ES/EN/PT utterance, otherwise the interface language if it
//!    is ES/EN/PT. Otherwise there is no retry.
//!
//! The retry replaces the first result only if it is clear speech in a
//! prior-language script ([`accept_retry`]). It is **untrusted text**,
//! exactly like a first transcript: it goes through the same understanding,
//! policy and confirmation, and the first transcript stays visible next to
//! it. Nothing here selects an application, a tool or an approval.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::language::LanguageTag;
use super::transcript::{self, Transcript, Verdict};

/// SERSHI's expected conversational languages (the prior).
pub const PRIOR: [&str; 3] = ["es", "en", "pt"];

/// Utterances up to this many words, or this much speech, are "short".
const SHORT_WORDS: usize = 6;
const SHORT_SPEECH_MS: u32 = 3_500;
/// A detection counts as recent conversational evidence for this long.
const RECENT_FOR: Duration = Duration::from_secs(10 * 60);
/// Only first-pass detections at least this confident are remembered.
const RECENT_MIN_CONFIDENCE: f32 = 0.7;
const RECENT_CAPACITY: usize = 5;

/// Whether a language belongs to the prior.
pub fn in_prior(language: &LanguageTag) -> bool {
    PRIOR.contains(&language.primary())
}

/// Letters Spanish, English and Portuguese write with (case-insensitive).
fn prior_letter(c: char) -> bool {
    let c = c.to_lowercase().next().unwrap_or(c);
    c.is_ascii_alphabetic() || "áéíóúüñãõâêôàç".contains(c)
}

/// Whether `text` contains a letter none of the prior languages uses
/// (Cyrillic, Greek, CJK, Icelandic ð/þ/æ, ø, ł…). Digits, punctuation and
/// symbols don't count.
pub fn foreign_letters(text: &str) -> bool {
    text.chars().any(|c| c.is_alphabetic() && !prior_letter(c))
}

/// Whether an utterance is short: a command rather than a conversation.
pub fn is_short(text: &str, speech_ms: Option<u32>) -> bool {
    transcript::clean(text).split_whitespace().count() <= SHORT_WORDS
        || speech_ms.is_some_and(|ms| ms <= SHORT_SPEECH_MS)
}

/// What SERSHI knows about the conversation besides the audio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageContext {
    /// The interface language (e.g. `es-419`).
    pub interface: Option<LanguageTag>,
    /// The last confidently detected prior language ([`RecentLanguages`]).
    pub recent: Option<LanguageTag>,
}

impl LanguageContext {
    /// The conversation's language: the recent one, else the interface
    /// language if SERSHI speaks it, else none.
    pub fn language(&self) -> Option<LanguageTag> {
        self.recent
            .iter()
            .chain(self.interface.iter())
            .find(|l| in_prior(l))
            .and_then(|l| LanguageTag::parse(l.primary()).ok())
    }
}

/// One first-pass recognition, as the retry policy sees it.
#[derive(Debug, Clone, Copy)]
pub struct FirstPass<'a> {
    pub transcript: &'a Transcript,
    /// Measured speech length, when known.
    pub speech_ms: Option<u32>,
    /// The conversation language is Automatic (detection ran).
    pub automatic: bool,
    /// SERSHI's deterministic tiers already understand the transcript.
    pub resolves: bool,
}

/// The language of the single second pass, or `None` (keep the first).
/// Called once, on the first pass only: a retry is never retried.
pub fn plan_retry(first: FirstPass<'_>, context: &LanguageContext) -> Option<LanguageTag> {
    let detected = first.transcript.language.as_ref()?;
    if !first.automatic
        || first.resolves
        || in_prior(detected)
        || !is_short(&first.transcript.text, first.speech_ms)
    {
        return None;
    }
    context.language()
}

/// The retry replaces the first result only if it is clear speech written
/// in a prior-language script. Returns the accepted text.
pub fn accept_retry(retry: &Transcript) -> Option<String> {
    match transcript::assess(retry) {
        Verdict::Accept(text) if !foreign_letters(&text) => Some(text),
        _ => None,
    }
}

/// Recently, *confidently* detected prior languages (session memory only,
/// bounded, never persisted). Retry results are never recorded, so a retry
/// cannot reinforce itself and lock the conversation into one language.
#[derive(Debug, Default)]
pub struct RecentLanguages {
    entries: VecDeque<(Instant, LanguageTag)>,
}

impl RecentLanguages {
    /// Records an Automatic first-pass detection (only confident prior ones).
    pub fn observe(&mut self, first: &Transcript, now: Instant) {
        let Some(language) = first.language.as_ref() else {
            return;
        };
        if !in_prior(language) || first.confidence < RECENT_MIN_CONFIDENCE {
            return;
        }
        if self.entries.len() == RECENT_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back((now, language.clone()));
    }

    /// The most recent language still within [`RECENT_FOR`].
    pub fn recent(&self, now: Instant) -> Option<LanguageTag> {
        self.entries
            .iter()
            .rev()
            .find(|(at, _)| now.saturating_duration_since(*at) <= RECENT_FOR)
            .map(|(_, l)| l.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(s: &str) -> LanguageTag {
        LanguageTag::parse(s).unwrap()
    }

    fn heard(text: &str, language: &str, confidence: f32) -> Transcript {
        Transcript {
            text: text.to_owned(),
            language: Some(tag(language)),
            no_speech_probability: 0.0,
            confidence,
        }
    }

    fn first(t: &Transcript, resolves: bool) -> FirstPass<'_> {
        FirstPass {
            transcript: t,
            speech_ms: Some(1_400),
            automatic: true,
            resolves,
        }
    }

    fn spanish_ui() -> LanguageContext {
        LanguageContext {
            interface: Some(tag("es-419")),
            recent: None,
        }
    }

    /// The physical misdetections (Gate 3C and 3C.1 runs), none of which
    /// SERSHI understands as heard: each gets exactly one Spanish pass in a
    /// Spanish conversation — whatever Whisper's confidence in the wrong
    /// language was (measured 0.97–0.99 for Icelandic).
    #[test]
    fn unresolved_short_misdetections_retry_in_the_conversation_language() {
        for (text, language) in [
            ("Ólug.", "is"),
            ("Ári olduk.", "is"),
            ("Ári vörð.", "is"),
            ("Ári og Óllug", "is"),
            ("Ári Óðluk", "is"),
            ("Og ég kýra á lýr Google Chrome", "is"),
            ("Kero árir krum", "is"),
            ("Пон Мекром", "ru"),
            ("Пон мекром", "ru"),
        ] {
            let t = heard(text, language, 0.56);
            assert_eq!(
                plan_retry(first(&t, false), &spanish_ui()),
                Some(tag("es")),
                "{text}"
            );
        }
    }

    /// Physical 3C.1: "Open Chrome", "Hópinn, Outlook" and "Oye, quiero
    /// abrir Google Chrome" were detected as Icelandic/Arabic but already
    /// understood. They must pay nothing.
    #[test]
    fn a_transcript_already_understood_is_never_retried() {
        for (text, language) in [
            ("Open Chrome.", "is"),
            ("Hópinn, Outlook.", "is"),
            ("Oye, quiero abrir Google Chrome.", "ar"),
        ] {
            let t = heard(text, language, 0.8);
            assert_eq!(plan_retry(first(&t, true), &spanish_ui()), None, "{text}");
        }
    }

    #[test]
    fn the_conversation_chooses_the_language_never_whisper() {
        let t = heard("Ári Óðluk", "is", 0.9);
        // The last confident conversation language comes first…
        let recent_en = LanguageContext {
            interface: Some(tag("es-419")),
            recent: Some(tag("en")),
        };
        assert_eq!(plan_retry(first(&t, false), &recent_en), Some(tag("en")));
        // …then the interface language, reduced to the language subtag.
        assert_eq!(plan_retry(first(&t, false), &spanish_ui()), Some(tag("es")));
        let pt_ui = LanguageContext {
            interface: Some(tag("pt-BR")),
            recent: None,
        };
        assert_eq!(plan_retry(first(&t, false), &pt_ui), Some(tag("pt")));
        // An interface SERSHI does not converse in names nothing: no retry.
        let other_ui = LanguageContext {
            interface: Some(tag("fr")),
            recent: None,
        };
        assert_eq!(plan_retry(first(&t, false), &other_ui), None);
        assert_eq!(
            plan_retry(first(&t, false), &LanguageContext::default()),
            None
        );
    }

    #[test]
    fn supported_languages_are_never_retried_so_switching_stays_free() {
        for (text, language) in [
            ("Abre Outlook", "es"),
            ("Open Outlook", "en"),
            ("Abra o Outlook", "pt"),
            // A garbled transcript in a supported language is a recognition
            // error, not a language error: no retry (physical "A reward.").
            ("A reward.", "es"),
        ] {
            let t = heard(text, language, 0.57);
            assert_eq!(plan_retry(first(&t, false), &spanish_ui()), None, "{text}");
        }
    }

    #[test]
    fn real_foreign_conversation_is_not_retried() {
        // Long speech in another language is a deliberate switch.
        let italian = heard(
            "Vorrei sapere quanta memoria sta usando il mio computer adesso per favore",
            "it",
            0.92,
        );
        let long = FirstPass {
            transcript: &italian,
            speech_ms: Some(5_200),
            automatic: true,
            resolves: false,
        };
        assert_eq!(plan_retry(long, &spanish_ui()), None);
    }

    #[test]
    fn fixed_languages_are_never_stabilized() {
        let t = heard("Ári Óðluk", "is", 0.9);
        let fixed = FirstPass {
            automatic: false,
            ..first(&t, false)
        };
        assert_eq!(plan_retry(fixed, &spanish_ui()), None);
        let mut unknown = heard("Пон Мекром", "ru", 0.5);
        unknown.language = None;
        assert_eq!(plan_retry(first(&unknown, false), &spanish_ui()), None);
    }

    /// No loop: the retry's own result carries the forced (supported)
    /// language, so the policy can never plan another pass after it.
    #[test]
    fn a_retry_is_never_retried() {
        let retried = heard("Ponme Chrome", "es", 0.9);
        assert_eq!(plan_retry(first(&retried, false), &spanish_ui()), None);
        let still_garbled = heard("Pon Micron", "es", 0.45);
        assert_eq!(
            plan_retry(first(&still_garbled, false), &spanish_ui()),
            None
        );
    }

    #[test]
    fn retries_must_be_clear_prior_script_speech() {
        assert_eq!(
            accept_retry(&heard("Ponme Chrome.", "es", 0.8)).as_deref(),
            Some("Ponme Chrome.")
        );
        assert_eq!(accept_retry(&heard("Пон Мекром", "es", 0.9)), None);
        assert_eq!(
            accept_retry(&heard("Ponme Chrome", "es", 0.2)),
            None,
            "unclear"
        );
        assert_eq!(accept_retry(&heard("", "es", 0.9)), None);
        assert!(foreign_letters("Ári Óðluk"), "ð is not Spanish");
        assert!(!foreign_letters("Ári og Óllug"), "Á, Ó are Spanish letters");
    }

    #[test]
    fn recent_languages_switch_freely_and_ignore_weak_detections() {
        let t0 = Instant::now();
        let mut recent = RecentLanguages::default();
        recent.observe(&heard("Abre Outlook", "es", 0.95), t0);
        assert_eq!(recent.recent(t0), Some(tag("es")));
        // ES → EN → PT: each confident first pass becomes the recent one.
        recent.observe(&heard("Open Chrome", "en", 0.9), t0);
        assert_eq!(recent.recent(t0), Some(tag("en")));
        recent.observe(&heard("Abra o Chrome", "pt", 0.9), t0);
        assert_eq!(recent.recent(t0), Some(tag("pt")));
        // Weak or out-of-prior detections are not evidence: a previous
        // low-confidence language is never reused.
        recent.observe(&heard("Пон Мекром", "ru", 0.99), t0);
        recent.observe(&heard("Open", "en", 0.4), t0);
        assert_eq!(recent.recent(t0), Some(tag("pt")));
        // It fades.
        assert_eq!(
            recent.recent(t0 + RECENT_FOR + Duration::from_secs(1)),
            None
        );
    }
}
