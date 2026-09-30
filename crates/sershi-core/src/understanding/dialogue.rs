//! Session conversation state: the question SERSHI is waiting on and a few
//! recent turns. Rust-authoritative, in memory only, bounded, and never
//! written to disk (persistent memory is a later, dedicated gate).
//!
//! Context improves understanding; it never grants authority. A
//! clarification only narrows *which trusted application* a request means.
//! The answer then goes through the normal pipeline — policy, and the
//! trusted confirmation window for sensitive actions — exactly like a typed
//! command. "Yes" to "Did you mean Word?" is not an approval of anything.

use std::collections::VecDeque;

use serde::Serialize;

use super::AppAction;
use super::grammar::{Token, tokens};
use super::similarity::similarity;
use crate::apps::ApplicationSummary;
use crate::apps::normalize::{normalize, words};

/// How long SERSHI waits for the answer to a clarification. Long enough to
/// hear the question and answer by voice; short enough that a stale
/// question can never be answered minutes later.
pub const CLARIFICATION_TTL_MS: u64 = 60_000;
/// Recent turns kept for context.
pub const MAX_TURNS: usize = 4;
/// Turns older than this are not used as context.
pub const TURN_TTL_MS: u64 = 5 * 60_000;
/// Longest remembered text of a turn (memory only).
const MAX_TURN_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ClarificationKind {
    /// Several trusted applications match: "Which one?"
    ChooseApplication,
    /// One likely application from an imperfect request: "Did you mean …?"
    DidYouMean,
    /// The action is clear but the application is not: "Which application?"
    WhichApplication,
    /// Several applications were named at once: "Which one first?"
    MultipleTargets,
}

/// A question SERSHI asks instead of guessing. Candidates are trusted
/// catalog entries (no paths).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct Clarification {
    pub kind: ClarificationKind,
    pub action: AppAction,
    pub candidates: Vec<ApplicationSummary>,
}

impl Clarification {
    /// Canonical English phrasing (surfaces localize from the structure).
    pub fn canonical_text(&self) -> String {
        let verb = match self.action {
            AppAction::Open => "open",
            AppAction::Close => "close",
        };
        let names: Vec<&str> = self
            .candidates
            .iter()
            .map(|c| c.display_name.as_str())
            .collect();
        match self.kind {
            ClarificationKind::ChooseApplication => format!(
                "I found more than one match: {}. Which one do you want?",
                names.join(", ")
            ),
            ClarificationKind::DidYouMean => {
                format!(
                    "Did you mean {verb} {}?",
                    names.first().copied().unwrap_or("")
                )
            }
            ClarificationKind::WhichApplication => {
                format!("Which application do you want me to {verb}?")
            }
            ClarificationKind::MultipleTargets => format!(
                "I can {verb} one application at a time: {}. Which one?",
                names.join(" or ")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingClarification {
    pub clarification: Clarification,
    pub asked_at: u64,
    pub expires_at: u64,
}

/// What an earlier turn led to, for context (no raw audio, nothing stored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub at: u64,
    /// Normalized text, truncated. Memory only.
    pub text: String,
    /// A short structured summary, e.g. "opened Outlook".
    pub summary: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Dialogue {
    pending: Option<PendingClarification>,
    turns: VecDeque<Turn>,
}

impl Dialogue {
    /// The pending question, if it has not expired.
    pub fn pending(&self, now: u64) -> Option<&PendingClarification> {
        self.pending.as_ref().filter(|p| now < p.expires_at)
    }

    /// Removes and returns an expired question.
    pub fn expire(&mut self, now: u64) -> Option<PendingClarification> {
        if self.pending.as_ref().is_some_and(|p| now >= p.expires_at) {
            self.pending.take()
        } else {
            None
        }
    }

    pub fn ask(&mut self, clarification: Clarification, now: u64) {
        self.pending = Some(PendingClarification {
            clarification,
            asked_at: now,
            expires_at: now + CLARIFICATION_TTL_MS,
        });
    }

    /// Clears the pending question; returns whether there was one.
    pub fn clear_pending(&mut self) -> bool {
        self.pending.take().is_some()
    }

    pub fn record(&mut self, now: u64, text: &str, summary: Option<String>) {
        let text: String = normalize(text).chars().take(MAX_TURN_CHARS).collect();
        self.turns.push_back(Turn {
            at: now,
            text,
            summary,
        });
        while self.turns.len() > MAX_TURNS {
            self.turns.pop_front();
        }
    }

    /// Recent turns still relevant, oldest first.
    pub fn recent(&self, now: u64) -> impl Iterator<Item = &Turn> {
        self.turns
            .iter()
            .filter(move |t| now.saturating_sub(t.at) < TURN_TTL_MS)
    }

    /// Forgets everything (session end, dismissal).
    pub fn reset(&mut self) {
        self.pending = None;
        self.turns.clear();
    }
}

/// How an utterance answers a pending question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// One of the offered candidates (index into the candidate list).
    Choose(usize),
    /// "No", "never mind", "cancel".
    Cancel,
    /// Not an answer to the question.
    NoMatch,
}

#[rustfmt::skip]
const YES: &[&str] = &[
    "si", "yes", "sim", "yeah", "yep", "claro", "correcto", "exacto", "eso", "ese", "esa",
    "that", "this", "isso", "esse", "essa", "dale", "vale", "ok", "okay", "right", "correct",
    "sure", "por favor", "please", "ese mismo", "that one", "esse mesmo", "eso es", "si por favor",
    "yes please", "sim por favor", "si claro", "abrelo", "open it", "si abrelo",
];
#[rustfmt::skip]
const NO: &[&str] = &[
    "no", "nope", "nao", "cancel", "cancela", "cancelar", "cancele", "never mind", "nevermind",
    "olvidalo", "dejalo", "deja", "esquece", "esquecer", "deixa", "ninguno", "ninguna", "none",
    "nenhum", "nenhuma", "no gracias", "no thanks", "nao obrigado", "stop", "para",
    "mejor no", "forget it",
];
/// Ordinal words by position (1-based), in three languages.
#[rustfmt::skip]
const ORDINALS: &[(&str, usize)] = &[
    ("primero", 1), ("primera", 1), ("primer", 1), ("first", 1), ("primeiro", 1), ("1", 1),
    ("uno", 1), ("una", 1), ("one", 1), ("um", 1), ("uma", 1),
    ("segundo", 2), ("segunda", 2), ("second", 2), ("2", 2), ("dos", 2), ("two", 2),
    ("dois", 2), ("duas", 2),
    ("tercero", 3), ("tercera", 3), ("tercer", 3), ("third", 3), ("terceiro", 3), ("terceira", 3),
    ("3", 3), ("tres", 3), ("three", 3),
    ("cuarto", 4), ("cuarta", 4), ("fourth", 4), ("quarto", 4), ("quarta", 4), ("4", 4),
    ("cuatro", 4), ("four", 4), ("quatro", 4),
    ("quinto", 5), ("quinta", 5), ("fifth", 5), ("5", 5), ("cinco", 5), ("five", 5),
    ("sexto", 6), ("sexta", 6), ("sixth", 6), ("6", 6), ("seis", 6), ("six", 6),
];
const LAST: &[&str] = &["ultimo", "ultima", "last"];
/// Words around an answer that carry no choice ("el segundo", "the second
/// one", "número dos", "la de Windows").
#[rustfmt::skip]
const ANSWER_FILLER: &[&str] = &[
    "el", "la", "lo", "los", "las", "the", "o", "a", "os", "as", "one", "numero", "number",
    "opcion", "option", "opcao", "de", "do", "da", "please", "por", "favor", "quiero", "want",
    "quero", "i", "sershi", "oye", "hey", "ok", "okay", "pues", "bueno", "mejor",
];

/// Matches an utterance against the options of a pending question. Only
/// the offered candidates can be chosen: nothing here can name anything
/// else.
pub fn answer(pending: &Clarification, text: &str) -> Answer {
    let all = tokens(text);
    let folded = normalize(text);
    let n = pending.candidates.len();

    if NO.contains(&folded.as_str()) {
        return Answer::Cancel;
    }
    let single = n == 1 && pending.kind == ClarificationKind::DidYouMean;
    if single && YES.contains(&folded.as_str()) {
        return Answer::Choose(0);
    }

    let core: Vec<&Token<'_>> = all
        .iter()
        .filter(|t| !ANSWER_FILLER.contains(&t.key.as_str()))
        .collect();
    // Ordinals only when they are the whole answer ("la segunda", "the
    // second one", "número 2"), never inside a name.
    if core.len() == 1 {
        let key = core[0].key.as_str();
        if let Some((_, position)) = ORDINALS.iter().find(|(w, _)| *w == key)
            && *position <= n
            && pending.kind != ClarificationKind::WhichApplication
        {
            return Answer::Choose(position - 1);
        }
        if LAST.contains(&key) && n > 0 {
            return Answer::Choose(n - 1);
        }
    }

    // A candidate's name: exact, then the fewest extra words, then a close
    // spelling with a clear margin.
    let spoken: String = core
        .iter()
        .map(|t| t.key.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if spoken.is_empty() {
        return Answer::NoMatch;
    }
    let names: Vec<String> = pending
        .candidates
        .iter()
        .map(|c| normalize(&c.display_name))
        .collect();
    if let Some(i) = names
        .iter()
        .position(|name| *name == spoken || *name == folded)
    {
        return Answer::Choose(i);
    }
    let spoken_words: Vec<&str> = words(&spoken).collect();
    let mut subset: Vec<(usize, usize)> = names
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            let name_words: Vec<&str> = words(name).collect();
            spoken_words
                .iter()
                .all(|w| name_words.contains(w))
                .then(|| {
                    (
                        i,
                        name_words.len() - spoken_words.len().min(name_words.len()),
                    )
                })
        })
        .collect();
    subset.sort_by_key(|(_, extra)| *extra);
    match subset.as_slice() {
        [(i, _)] => return Answer::Choose(*i),
        [(i, a), (_, b), ..] if a < b => return Answer::Choose(*i),
        _ => {}
    }
    let mut scored: Vec<(usize, f32)> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (i, similarity(&spoken, name).combined))
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    match scored.as_slice() {
        [(i, top)] if *top >= 0.75 => Answer::Choose(*i),
        [(i, top), (_, second), ..] if *top >= 0.75 && top - second >= 0.1 => Answer::Choose(*i),
        _ => Answer::NoMatch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::AppSource;

    fn app(name: &str) -> ApplicationSummary {
        ApplicationSummary {
            id: crate::apps::slug(name),
            display_name: name.to_owned(),
            source: AppSource::StartMenu,
        }
    }

    fn powershell() -> Clarification {
        Clarification {
            kind: ClarificationKind::ChooseApplication,
            action: AppAction::Open,
            candidates: vec![
                app("Windows PowerShell"),
                app("Windows PowerShell ISE"),
                app("Anaconda PowerShell Prompt"),
                app("Windows PowerShell ISE (x86)"),
            ],
        }
    }

    #[test]
    fn answers_by_name_ordinal_or_cancel() {
        let q = powershell();
        for (text, expected) in [
            ("Windows PowerShell", Answer::Choose(0)),
            ("Windows PowerShell.", Answer::Choose(0)),
            ("PowerShell ISE", Answer::Choose(1)),
            ("La de Anaconda", Answer::Choose(2)),
            ("El primero", Answer::Choose(0)),
            ("La segunda", Answer::Choose(1)),
            ("The second one", Answer::Choose(1)),
            ("O segundo", Answer::Choose(1)),
            ("Número 2", Answer::Choose(1)),
            ("el último", Answer::Choose(3)),
            ("Never mind", Answer::Cancel),
            ("No", Answer::Cancel),
            ("Cancelar", Answer::Cancel),
            ("Esquece", Answer::Cancel),
            // Out of range, unrelated, or approval words: no choice.
            ("La quinta", Answer::NoMatch),
            ("Excel", Answer::NoMatch),
            ("Sí", Answer::NoMatch),
            ("Yes", Answer::NoMatch),
            ("Aprobar", Answer::NoMatch),
        ] {
            assert_eq!(answer(&q, text), expected, "{text}");
        }
    }

    #[test]
    fn yes_only_confirms_a_single_meaning() {
        let q = Clarification {
            kind: ClarificationKind::DidYouMean,
            action: AppAction::Open,
            candidates: vec![app("Word")],
        };
        for text in ["Yes", "Sí", "sim", "Eso", "Sí, por favor"] {
            assert_eq!(answer(&q, text), Answer::Choose(0), "{text}");
        }
        assert_eq!(answer(&q, "No"), Answer::Cancel);
        assert_eq!(answer(&q, "Word"), Answer::Choose(0));
    }

    #[test]
    fn pending_questions_expire_and_turns_are_bounded() {
        let mut d = Dialogue::default();
        d.ask(powershell(), 1_000);
        assert!(d.pending(1_000 + CLARIFICATION_TTL_MS - 1).is_some());
        assert!(d.pending(1_000 + CLARIFICATION_TTL_MS).is_none());
        assert!(d.expire(1_000 + CLARIFICATION_TTL_MS).is_some());
        assert!(!d.clear_pending());
        for i in 0..10 {
            d.record(i, &"x".repeat(500), None);
        }
        assert_eq!(d.recent(10).count(), MAX_TURNS);
        assert!(d.recent(10).all(|t| t.text.len() <= MAX_TURN_CHARS));
        assert_eq!(d.recent(10 + TURN_TTL_MS).count(), 0);
    }
}
