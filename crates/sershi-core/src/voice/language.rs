//! Conversation languages as BCP-47 tags.
//!
//! The conversation language (what SERSHI hears and answers) is independent
//! of the interface locale: a Spanish interface with an English conversation
//! must work. `None` everywhere means *automatic* (the recogniser detects
//! the spoken language). Adding a language is data, not a redesign.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated, normalised BCP-47 language tag (`en`, `es`, `pt-BR`, …).
/// Only the `language[-region]` subset is accepted: 2–3 letter language,
/// optional 2-letter or 3-digit region. Anything else is rejected rather than
/// guessed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, type = "string"))]
pub struct LanguageTag(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("not a supported BCP-47 language tag")]
pub struct InvalidLanguageTag;

/// Conversation languages offered in Settings (besides Automatic).
pub const CONVERSATION_LANGUAGES: [&str; 3] = ["en", "es", "pt"];

impl LanguageTag {
    pub fn parse(raw: &str) -> Result<Self, InvalidLanguageTag> {
        let mut parts = raw.trim().split(['-', '_']);
        let language = parts.next().ok_or(InvalidLanguageTag)?;
        if !(2..=3).contains(&language.len()) || !language.chars().all(|c| c.is_ascii_alphabetic())
        {
            return Err(InvalidLanguageTag);
        }
        let mut tag = language.to_ascii_lowercase();
        if let Some(region) = parts.next() {
            let letters = region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic());
            let digits = region.len() == 3 && region.chars().all(|c| c.is_ascii_digit());
            if !letters && !digits {
                return Err(InvalidLanguageTag);
            }
            tag.push('-');
            tag.push_str(&region.to_ascii_uppercase());
        }
        if parts.next().is_some() {
            return Err(InvalidLanguageTag);
        }
        Ok(Self(tag))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The language subtag alone (`pt` for `pt-BR`), which is what speech
    /// recognisers are hinted with.
    pub fn primary(&self) -> &str {
        self.0.split('-').next().unwrap_or(&self.0)
    }

    /// Whether two tags name the same language, ignoring region.
    pub fn same_language(&self, other: &LanguageTag) -> bool {
        self.primary() == other.primary()
    }
}

impl TryFrom<String> for LanguageTag {
    type Error = InvalidLanguageTag;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<LanguageTag> for String {
    fn from(tag: LanguageTag) -> Self {
        tag.0
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_language_and_region() {
        assert_eq!(LanguageTag::parse("ES").unwrap().as_str(), "es");
        assert_eq!(LanguageTag::parse("pt_br").unwrap().as_str(), "pt-BR");
        assert_eq!(LanguageTag::parse("es-419").unwrap().as_str(), "es-419");
        assert_eq!(LanguageTag::parse("pt-BR").unwrap().primary(), "pt");
    }

    #[test]
    fn rejects_anything_that_is_not_a_simple_tag() {
        for bad in [
            "", "e", "english", "en-", "en-USA", "en-US-x", "../en", "e1", "zh-Hans",
        ] {
            assert!(LanguageTag::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn deserialisation_validates() {
        assert!(serde_json::from_str::<LanguageTag>("\"es\"").is_ok());
        assert!(serde_json::from_str::<LanguageTag>("\"--rm\"").is_err());
    }

    #[test]
    fn offered_languages_are_valid_tags() {
        for tag in CONVERSATION_LANGUAGES {
            assert_eq!(LanguageTag::parse(tag).unwrap().as_str(), tag);
        }
    }
}
