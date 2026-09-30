//! The deterministic confidence policy. Every threshold that decides
//! between acting, asking and doing nothing lives here (docs/SEMANTIC.md
//! explains how they were chosen).
//!
//! False execution is worse than a question: below the acting thresholds
//! SERSHI asks "Did you mean …?" or "Which one?", and below the asking
//! thresholds it does nothing. Closing an application is never inferred
//! from an imperfect match — it is always asked — and even then it still
//! goes to the trusted confirmation window.

use super::AppAction;
use super::similarity::Similarity;
use crate::apps::ApplicationSummary;

/// A fuzzy/phonetic name match opens an application on its own at or above
/// this score…
pub const FUZZY_ACCEPT: f32 = 0.88;
/// …and only if the runner-up is at least this much lower.
pub const FUZZY_MARGIN: f32 = 0.08;
/// Below acceptance but at or above this, SERSHI asks "Did you mean …?".
pub const FUZZY_ASK: f32 = 0.70;
/// Candidates within this of the best (and above `FUZZY_ASK`) are offered
/// together ("Which one?") instead of one "Did you mean".
pub const FUZZY_TIE: f32 = 0.05;
/// Fuzzy matching needs at least this many characters in the query.
pub const FUZZY_MIN_CHARS: usize = 4;
/// Most candidates offered in one question.
pub const MAX_OFFERED: usize = 4;

/// A malformed first word counts as "open"/"close" at or above this
/// similarity to a known verb ("Apreer", "Afrið" → "abrir").
pub const VERB_LIKE: f32 = 0.70;

/// Semantic results: acting needs this combined score; asking needs
/// `SEMANTIC_ASK`. Weights: catalog evidence for the chosen application,
/// the model's (untrusted) confidence, and a verb actually heard.
pub const SEMANTIC_ACCEPT: f32 = 0.80;
pub const SEMANTIC_ASK: f32 = 0.55;
pub const W_EVIDENCE: f32 = 0.5;
pub const W_MODEL: f32 = 0.3;
pub const W_VERB: f32 = 0.2;
/// The model must be at least this confident for its answer to a pending
/// question or its "which application?" to be used.
pub const SEMANTIC_MIN_MODEL: f32 = 0.6;
/// Speech-recognition confidence below this lowers every semantic score by
/// `ASR_PENALTY`.
pub const ASR_LOW: f32 = 0.55;
pub const ASR_PENALTY: f32 = 0.15;

/// A trusted application with its match score.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub application: ApplicationSummary,
    pub similarity: Similarity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FuzzyDecision {
    /// Confident, unique: act (opening only).
    Accept(Scored),
    DidYouMean(Scored),
    Choose(Vec<ApplicationSummary>),
    Nothing,
}

/// Decides on a ranked (best first) list of fuzzy matches.
pub fn decide_fuzzy(action: AppAction, ranked: &[Scored]) -> FuzzyDecision {
    let Some(best) = ranked.first() else {
        return FuzzyDecision::Nothing;
    };
    let top = best.similarity.combined;
    let second = ranked.get(1).map_or(0.0, |s| s.similarity.combined);
    if top < FUZZY_ASK {
        return FuzzyDecision::Nothing;
    }
    let tied: Vec<&Scored> = ranked
        .iter()
        .filter(|s| s.similarity.combined >= FUZZY_ASK && top - s.similarity.combined <= FUZZY_TIE)
        .collect();
    if tied.len() > 1 {
        return FuzzyDecision::Choose(
            tied.iter()
                .take(MAX_OFFERED)
                .map(|s| s.application.clone())
                .collect(),
        );
    }
    if action == AppAction::Open && top >= FUZZY_ACCEPT && top - second >= FUZZY_MARGIN {
        FuzzyDecision::Accept(best.clone())
    } else {
        FuzzyDecision::DidYouMean(best.clone())
    }
}

/// The combined semantic score (see the weights above).
pub fn semantic_score(evidence: f32, model: f32, verb_heard: bool, asr: Option<f32>) -> f32 {
    let mut score = W_EVIDENCE * evidence.clamp(0.0, 1.0)
        + W_MODEL * model.clamp(0.0, 1.0)
        + if verb_heard { W_VERB } else { 0.0 };
    if asr.is_some_and(|a| a < ASR_LOW) {
        score -= ASR_PENALTY;
    }
    score
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticDecision {
    Act,
    Ask,
    Nothing,
}

/// Acting also needs a command word actually heard: a model reading a
/// statement ("Chrome es mi navegador favorito") as a request can at most
/// make SERSHI ask.
pub fn decide_semantic(action: AppAction, score: f32, verb_heard: bool) -> SemanticDecision {
    if action == AppAction::Open && verb_heard && score >= SEMANTIC_ACCEPT {
        SemanticDecision::Act
    } else if score >= SEMANTIC_ASK {
        SemanticDecision::Ask
    } else {
        SemanticDecision::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::AppSource;

    fn scored(name: &str, combined: f32) -> Scored {
        Scored {
            application: ApplicationSummary {
                id: crate::apps::slug(name),
                display_name: name.to_owned(),
                source: AppSource::StartMenu,
            },
            similarity: Similarity {
                lexical: combined,
                phonetic: combined,
                combined,
            },
        }
    }

    #[test]
    fn only_a_strong_unique_match_opens() {
        assert!(matches!(
            decide_fuzzy(
                AppAction::Open,
                &[scored("Excel", 0.9), scored("Edge", 0.5)]
            ),
            FuzzyDecision::Accept(_)
        ));
        // Strong but not clearly ahead: ask.
        assert!(matches!(
            decide_fuzzy(
                AppAction::Open,
                &[scored("Word", 0.9), scored("WordPad", 0.84)]
            ),
            FuzzyDecision::DidYouMean(_)
        ));
        // Two nearly equal: offer both.
        assert!(matches!(
            decide_fuzzy(AppAction::Open, &[scored("Word", 0.8), scored("WordPad", 0.78)]),
            FuzzyDecision::Choose(ref c) if c.len() == 2
        ));
        assert_eq!(
            decide_fuzzy(AppAction::Open, &[scored("Word", 0.5)]),
            FuzzyDecision::Nothing
        );
        assert_eq!(decide_fuzzy(AppAction::Open, &[]), FuzzyDecision::Nothing);
    }

    #[test]
    fn closing_is_never_inferred_from_an_imperfect_match() {
        assert!(matches!(
            decide_fuzzy(AppAction::Close, &[scored("Excel", 0.99)]),
            FuzzyDecision::DidYouMean(_)
        ));
        assert_eq!(
            decide_semantic(AppAction::Close, 1.0, true),
            SemanticDecision::Ask
        );
    }

    #[test]
    fn model_confidence_alone_cannot_reach_action() {
        // Full model confidence, no catalog evidence, no verb heard.
        let score = semantic_score(0.0, 1.0, false, None);
        assert_eq!(
            decide_semantic(AppAction::Open, score, false),
            SemanticDecision::Nothing
        );
        // Full evidence and confidence but no command word: ask.
        let score = semantic_score(1.0, 1.0, false, None);
        assert_eq!(
            decide_semantic(AppAction::Open, score, false),
            SemanticDecision::Ask
        );
        // Even with a verb, weak evidence only asks.
        let score = semantic_score(0.4, 1.0, true, None);
        assert_eq!(
            decide_semantic(AppAction::Open, score, true),
            SemanticDecision::Ask
        );
        // Strong evidence, a verb and a confident model: act.
        let score = semantic_score(0.9, 0.9, true, None);
        assert_eq!(
            decide_semantic(AppAction::Open, score, true),
            SemanticDecision::Act
        );
        // Poorly recognised speech asks instead.
        let score = semantic_score(0.9, 0.9, true, Some(0.3));
        assert_eq!(
            decide_semantic(AppAction::Open, score, true),
            SemanticDecision::Ask
        );
    }
}
