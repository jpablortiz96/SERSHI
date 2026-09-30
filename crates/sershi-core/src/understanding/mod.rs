//! Natural command understanding (Gate 3C).
//!
//! **Natural language in. Structured intent out. Policy still decides.**
//!
//! ```text
//! text ─ Tier 0 normalize (apps::normalize)
//!      ─ pending question? answer it from the offered candidates only
//!      ─ Tier 1 grammar frame (grammar) ─ trusted catalog: exact, alias,
//!        vendor name, prefix, words
//!      ─ Tier 2 fuzzy + phonetic ranking (similarity) → policy thresholds
//!      ─ system keywords (intent::KeywordIntentResolver)
//!      ─ bare application name, malformed verb, several names
//!      ─ Tier 3 local semantic model (semantic), if installed
//!      → Intent: ToolCall | Clarify | Answer | Cancel | NotUnderstood
//! ```
//!
//! The fast path stays fast: a request the deterministic tiers resolve
//! never reaches the model. Every tier can only *select* a trusted catalog
//! entry; the resulting [`ToolCall`] names it by id and the tool resolves
//! that id through the catalog again. Nothing here launches, closes or
//! approves anything: the result enters the same executor, policy and
//! trusted confirmation as a typed command.

pub mod dialogue;
pub mod grammar;
pub mod models;
pub mod policy;
pub mod semantic;
pub mod similarity;
pub mod status;

#[cfg(test)]
mod tests;

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use serde_json::json;

pub use dialogue::{
    Answer, CLARIFICATION_TTL_MS, Clarification, ClarificationKind, Dialogue, PendingClarification,
};
pub use grammar::AppAction;
pub use semantic::{RouterError, SemanticIntent, SemanticRouterPort};

use self::grammar::{Frame, Token};
use self::policy::{FuzzyDecision, Scored, SemanticDecision};
use self::semantic::{PendingQuestion, SemanticCandidate, SemanticOption, SemanticRequest};
use self::similarity::similarity;
use crate::apps::catalog::CatalogNames;
use crate::apps::normalize::normalize;
use crate::apps::tools::{CLOSE_APPLICATION, OPEN_APPLICATION};
use crate::apps::{ApplicationManager, ApplicationSummary, MatchKind, Resolution};
use crate::ids::ToolId;
use crate::intent::{AnswerTopic, Intent, IntentResolver};
use crate::tool::{CallOrigin, ToolCall};

/// Read access to the trusted application catalog.
pub trait ApplicationDirectory: Send + Sync + Debug {
    /// `None` when the catalog cannot be read (unsupported platform, scan
    /// failure).
    fn resolve(&self, query: &str) -> Option<Resolution>;
    fn names(&self) -> Vec<CatalogNames>;
}

impl ApplicationDirectory for ApplicationManager {
    fn resolve(&self, query: &str) -> Option<Resolution> {
        ApplicationManager::resolve(self, query).ok()
    }
    fn names(&self) -> Vec<CatalogNames> {
        ApplicationManager::names(self).unwrap_or_default()
    }
}

/// How an interpretation was reached (diagnostics and audit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ResolutionTier {
    /// System keywords, greetings, cancellation, negation.
    Keyword,
    /// The exact name of a trusted application.
    Exact,
    /// A trusted alias or vendor-qualified name.
    Alias,
    /// A whole-word prefix or word match in the catalog.
    Catalog,
    /// Spelling similarity.
    Fuzzy,
    /// Spelling raised by sound similarity.
    Phonetic,
    /// A malformed command word before a trusted name ("Apreer Chrome").
    VerbRepair,
    /// The answer to SERSHI's question.
    Context,
    /// The local semantic model.
    Semantic,
    /// Nothing resolved.
    None,
}

/// Whether and how the local model took part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum SemanticUse {
    /// The deterministic tiers were enough (the fast path).
    NotNeeded,
    /// No model installed or enabled.
    NotInstalled,
    Used,
    /// The model failed or timed out; deterministic results were kept.
    Failed,
    /// The model answered, but its output was invalid or not supported by
    /// the evidence.
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CandidateScore {
    pub application: ApplicationSummary,
    pub score: f32,
}

/// "SERSHI understood: Open Microsoft Word" — shown when the result differs
/// from the words that were said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct UnderstoodAs {
    pub action: AppAction,
    pub application: ApplicationSummary,
}

/// Developer diagnostics for one request. Transient: returned with the
/// outcome, never recorded in the activity log or stored.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct UnderstandingTrace {
    pub normalized: String,
    pub tier: ResolutionTier,
    pub candidates: Vec<CandidateScore>,
    /// SERSHI's own (deterministic) confidence in the result, 0–1.
    pub confidence: f32,
    pub semantic: SemanticUse,
    pub semantic_ms: Option<u32>,
    /// Time spent understanding, including the model if used.
    pub understanding_ms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Typed,
    Voice,
}

/// One request as understanding sees it.
#[derive(Debug, Clone, Copy)]
pub struct Utterance<'a> {
    pub text: &'a str,
    pub source: InputSource,
    /// Mean token probability from speech recognition, if spoken.
    pub asr_confidence: Option<f32>,
    pub language: Option<&'a str>,
}

impl<'a> Utterance<'a> {
    pub fn typed(text: &'a str) -> Self {
        Self {
            text,
            source: InputSource::Typed,
            asr_confidence: None,
            language: None,
        }
    }
}

/// What to do with the pending question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingChange {
    Keep,
    Clear,
    /// Ask this question (replaces any pending one).
    Ask(Clarification),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Interpretation {
    pub intent: Intent,
    pub understood: Option<UnderstoodAs>,
    pub trace: UnderstandingTrace,
    pub pending: PendingChange,
}

/// The understanding pipeline. Holds no conversation state itself: the
/// [`Dialogue`] is owned by the service and passed in.
#[derive(Debug)]
pub struct Understanding {
    keywords: Box<dyn IntentResolver>,
    apps: Option<Arc<dyn ApplicationDirectory>>,
    router: Option<Arc<dyn SemanticRouterPort>>,
}

/// Most words in a bare application name or a target worth resolving.
const MAX_NAME_WORDS: usize = 5;
/// Most n-gram length used to find candidates for the model.
const MAX_NGRAM: usize = 4;
/// Minimum similarity for a candidate offered to the model.
const OPTION_MIN: f32 = 0.45;

impl Understanding {
    pub fn new(keywords: Box<dyn IntentResolver>) -> Self {
        Self {
            keywords,
            apps: None,
            router: None,
        }
    }

    pub fn with_applications(mut self, apps: Arc<dyn ApplicationDirectory>) -> Self {
        self.apps = Some(apps);
        self
    }

    pub fn with_router(mut self, router: Arc<dyn SemanticRouterPort>) -> Self {
        self.router = Some(router);
        self
    }

    pub fn set_applications(&mut self, apps: Arc<dyn ApplicationDirectory>) {
        self.apps = Some(apps);
    }

    pub fn set_router(&mut self, router: Option<Arc<dyn SemanticRouterPort>>) {
        self.router = router;
    }

    /// Lets a runtime load its model while the user is still speaking.
    pub fn prepare(&self) {
        if let Some(router) = &self.router {
            router.prepare();
        }
    }

    pub fn interpret(&self, u: &Utterance<'_>, dialogue: &Dialogue, now: u64) -> Interpretation {
        let started = Instant::now();
        let mut run = Run {
            u,
            tokens: grammar::tokens(u.text),
            trace: UnderstandingTrace {
                normalized: normalize(u.text).chars().take(200).collect(),
                tier: ResolutionTier::None,
                candidates: Vec::new(),
                confidence: 0.0,
                semantic: SemanticUse::NotNeeded,
                semantic_ms: None,
                understanding_ms: 0.0,
            },
            understood: None,
        };
        let pending = dialogue.pending(now).map(|p| &p.clarification);
        let (intent, change) = self.decide(&mut run, pending, dialogue, now);
        run.trace.understanding_ms = started.elapsed().as_secs_f32() * 1000.0;
        Interpretation {
            intent,
            understood: run.understood,
            trace: run.trace,
            pending: change,
        }
    }

    fn decide(
        &self,
        run: &mut Run<'_, '_>,
        pending: Option<&Clarification>,
        dialogue: &Dialogue,
        now: u64,
    ) -> (Intent, PendingChange) {
        if let Some(question) = pending {
            match self.answer(run, question) {
                Some(Intent::Cancel) => return (Intent::Cancel, PendingChange::Clear),
                Some(intent) => return (intent, PendingChange::Clear),
                None => {}
            }
        }
        let intent = self.fresh(run, pending, dialogue, now);
        match (&intent, pending) {
            (Intent::Clarify(c), _) => {
                let c = c.clone();
                (intent, PendingChange::Ask(c))
            }
            // Not an answer and nothing else understood: ask again.
            (Intent::NotUnderstood, Some(question)) => {
                run.trace.tier = ResolutionTier::Context;
                (Intent::Clarify(question.clone()), PendingChange::Keep)
            }
            (_, Some(_)) => (intent, PendingChange::Clear),
            (_, None) => (intent, PendingChange::Keep),
        }
    }

    /// Resolves an utterance as the answer to a pending question.
    fn answer(&self, run: &mut Run<'_, '_>, question: &Clarification) -> Option<Intent> {
        match dialogue::answer(question, run.u.text) {
            Answer::Cancel => {
                run.trace.tier = ResolutionTier::Context;
                Some(Intent::Cancel)
            }
            Answer::Choose(i) => {
                let app = question.candidates.get(i)?.clone();
                run.trace.tier = ResolutionTier::Context;
                run.trace.confidence = 1.0;
                Some(app_call(question.action, &app, CallOrigin::User))
            }
            Answer::NoMatch if question.kind == ClarificationKind::WhichApplication => {
                // The answer names the application ("Chrome", "el Excel").
                let name = grammar::bare(&run.tokens);
                if name.is_empty() || grammar::parse_tokens(&run.tokens) != Frame::None {
                    return None;
                }
                match self.target(run, question.action, &name) {
                    Target::Act(intent) | Target::Ask(intent) => Some(intent),
                    Target::Unresolved | Target::NoCatalog => None,
                }
            }
            Answer::NoMatch => None,
        }
    }

    /// Understands an utterance on its own (no pending answer matched).
    fn fresh(
        &self,
        run: &mut Run<'_, '_>,
        pending: Option<&Clarification>,
        dialogue: &Dialogue,
        now: u64,
    ) -> Intent {
        let frame = grammar::parse_tokens(&run.tokens);
        let keyword = || self.keywords.resolve(run.u.text);
        match frame {
            Frame::Negated => {
                run.trace.tier = ResolutionTier::Keyword;
                run.trace.confidence = 1.0;
                return Intent::Answer(AnswerTopic::NoAction);
            }
            Frame::Command { action, target } => {
                if target.is_empty() {
                    if self.apps.is_some() {
                        return clarify(ClarificationKind::WhichApplication, action, vec![]);
                    }
                    return Intent::NotUnderstood;
                }
                match self.target(run, action, &target) {
                    Target::Act(intent) | Target::Ask(intent) => return intent,
                    Target::NoCatalog => return raw_call(action, &target),
                    Target::Unresolved => {}
                }
                // "open task manager to check memory" was handled above; a
                // target that is not an application may still be a system
                // question ("quiero revisar la memoria").
                match keyword() {
                    Intent::UseTool(call) if !is_app_tool(&call) => {
                        run.trace.tier = ResolutionTier::Keyword;
                        return Intent::UseTool(call);
                    }
                    intent @ Intent::NotYetAvailable(_) => {
                        run.trace.tier = ResolutionTier::Keyword;
                        return intent;
                    }
                    _ => {}
                }
                if let Some(intent) = self.several(run, action, &target) {
                    return intent;
                }
                if let Some(intent) = self.semantic(run, pending, dialogue, now) {
                    return intent;
                }
                let words = grammar::tokens(&target).len();
                if words > 4 || target_has_verb(&target) {
                    return clarify(ClarificationKind::WhichApplication, action, vec![]);
                }
                // An ordinary "not found": the tool says so, as before.
                return raw_call(action, &target);
            }
            Frame::Wish { target } => {
                if let Some(app) = self.strict(&target) {
                    run.trace.tier = ResolutionTier::Exact;
                    run.trace.confidence = 1.0;
                    return app_call(AppAction::Open, &app, CallOrigin::User);
                }
            }
            Frame::None => {}
        }

        match keyword() {
            Intent::NotUnderstood => {}
            // Application commands are the grammar's job (above).
            Intent::UseTool(call) if is_app_tool(&call) => {}
            intent => {
                run.trace.tier = ResolutionTier::Keyword;
                run.trace.confidence = 1.0;
                return intent;
            }
        }
        if self.apps.is_some() {
            if let Some(intent) = self.bare_name(run) {
                return intent;
            }
            if let Some(intent) = self.repaired_verb(run) {
                return intent;
            }
        }
        if let Some(intent) = self.semantic(run, pending, dialogue, now) {
            return intent;
        }
        Intent::NotUnderstood
    }

    /// Resolves a target phrase for `action` against the catalog.
    fn target(&self, run: &mut Run<'_, '_>, action: AppAction, target: &str) -> Target {
        let Some(apps) = &self.apps else {
            return Target::NoCatalog;
        };
        match apps.resolve(target) {
            None => Target::NoCatalog,
            Some(Resolution::Found {
                application,
                matched,
            }) => {
                run.trace.tier = match matched {
                    MatchKind::Exact => ResolutionTier::Exact,
                    MatchKind::Alias => ResolutionTier::Alias,
                    MatchKind::Prefix | MatchKind::Words => ResolutionTier::Catalog,
                };
                run.trace.confidence = 1.0;
                Target::Act(app_call(action, &application.summary(), CallOrigin::User))
            }
            Some(Resolution::Ambiguous(candidates)) => {
                run.trace.tier = ResolutionTier::Catalog;
                Target::Ask(clarify(
                    ClarificationKind::ChooseApplication,
                    action,
                    candidates.into_iter().take(policy::MAX_OFFERED).collect(),
                ))
            }
            Some(Resolution::NotFound) => {
                let query = normalize(target);
                if query.chars().count() < policy::FUZZY_MIN_CHARS
                    || grammar::tokens(target).len() > MAX_NAME_WORDS
                {
                    return Target::Unresolved;
                }
                let ranked = rank(&apps.names(), &query);
                run.note_candidates(&ranked);
                match policy::decide_fuzzy(action, &ranked) {
                    FuzzyDecision::Accept(best) => {
                        run.trace.tier = fuzzy_tier(&best);
                        run.trace.confidence = best.similarity.combined;
                        run.understood = Some(UnderstoodAs {
                            action,
                            application: best.application.clone(),
                        });
                        Target::Act(app_call(action, &best.application, CallOrigin::User))
                    }
                    FuzzyDecision::DidYouMean(best) => {
                        run.trace.tier = fuzzy_tier(&best);
                        run.trace.confidence = best.similarity.combined;
                        Target::Ask(clarify(
                            ClarificationKind::DidYouMean,
                            action,
                            vec![best.application],
                        ))
                    }
                    FuzzyDecision::Choose(candidates) => {
                        run.trace.tier = ResolutionTier::Fuzzy;
                        Target::Ask(clarify(
                            ClarificationKind::ChooseApplication,
                            action,
                            candidates,
                        ))
                    }
                    FuzzyDecision::Nothing => Target::Unresolved,
                }
            }
        }
    }

    /// An exact or alias match only (bare names and wishes).
    fn strict(&self, name: &str) -> Option<ApplicationSummary> {
        let apps = self.apps.as_ref()?;
        if name.is_empty() || grammar::tokens(name).len() > MAX_NAME_WORDS {
            return None;
        }
        match apps.resolve(name)? {
            Resolution::Found {
                application,
                matched: MatchKind::Exact | MatchKind::Alias,
            } => Some(application.summary()),
            _ => None,
        }
    }

    /// "Outlook" on its own opens Outlook: only an exact or alias match of
    /// the whole utterance, never a word inside a sentence.
    fn bare_name(&self, run: &mut Run<'_, '_>) -> Option<Intent> {
        let name = grammar::bare(&run.tokens);
        if let Some(app) = self.strict(&name) {
            run.trace.tier = ResolutionTier::Exact;
            run.trace.confidence = 1.0;
            return Some(app_call(AppAction::Open, &app, CallOrigin::User));
        }
        self.several(run, AppAction::Open, &name)
    }

    /// "Outlook, Google Chrome" or "abre Outlook y Chrome": one at a time.
    fn several(&self, run: &mut Run<'_, '_>, action: AppAction, text: &str) -> Option<Intent> {
        let pieces: Vec<String> = split_names(text);
        if pieces.len() < 2 {
            return None;
        }
        let apps: Vec<ApplicationSummary> = pieces
            .iter()
            .map(|p| self.strict(p))
            .collect::<Option<Vec<_>>>()?;
        run.trace.tier = ResolutionTier::Exact;
        Some(clarify(
            ClarificationKind::MultipleTargets,
            action,
            apps.into_iter().take(policy::MAX_OFFERED).collect(),
        ))
    }

    /// "Apreer Google Chrome", "Afrið Google Chrome": an unknown first word
    /// before an exact trusted name.
    fn repaired_verb(&self, run: &mut Run<'_, '_>) -> Option<Intent> {
        let start = grammar::skip_fillers(&run.tokens);
        let (first, rest) = run.tokens.get(start..)?.split_first()?;
        if first.key.chars().count() < 3 || rest.is_empty() {
            return None;
        }
        let open = verb_likeness(&first.key, grammar::OPEN_VERBS, OPEN_LIKE);
        let close = verb_likeness(&first.key, grammar::CLOSE_VERBS, CLOSE_LIKE);
        let name = grammar::target(rest);
        let Some(app) = self.strict(&name) else {
            // A command word with an unrecognisable name: ask which one.
            if open >= policy::VERB_LIKE && open >= close && rest.len() <= 3 {
                run.trace.tier = ResolutionTier::VerbRepair;
                return Some(clarify(
                    ClarificationKind::WhichApplication,
                    AppAction::Open,
                    vec![],
                ));
            }
            return None;
        };
        run.trace.tier = ResolutionTier::VerbRepair;
        run.trace.confidence = open.max(close);
        if open >= policy::VERB_LIKE && open >= close {
            run.understood = Some(UnderstoodAs {
                action: AppAction::Open,
                application: app.clone(),
            });
            return Some(app_call(AppAction::Open, &app, CallOrigin::User));
        }
        let action = if close >= policy::VERB_LIKE {
            AppAction::Close
        } else {
            AppAction::Open
        };
        // An unknown word before a name: likely a mangled command, but only
        // asked, never done ("Average Google Chrome").
        if rest.len() <= MAX_NAME_WORDS {
            return Some(clarify(ClarificationKind::DidYouMean, action, vec![app]));
        }
        None
    }

    /// Tier 3. Returns `None` to fall through to "not understood".
    fn semantic(
        &self,
        run: &mut Run<'_, '_>,
        pending: Option<&Clarification>,
        dialogue: &Dialogue,
        now: u64,
    ) -> Option<Intent> {
        let Some(router) = &self.router else {
            run.trace.semantic = SemanticUse::NotInstalled;
            return None;
        };
        let (request, options, evidence) = self.build_request(run, pending, dialogue, now)?;
        let started = Instant::now();
        let result = router.route(&request);
        run.trace.semantic_ms =
            Some(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX));
        let candidate = match result {
            Ok(c) => c,
            Err(RouterError::NotInstalled) => {
                run.trace.semantic = SemanticUse::NotInstalled;
                return None;
            }
            Err(RouterError::InvalidOutput) => {
                run.trace.semantic = SemanticUse::Rejected;
                return None;
            }
            Err(_) => {
                run.trace.semantic = SemanticUse::Failed;
                return None;
            }
        };
        run.trace.semantic = SemanticUse::Used;
        let decided = self.weigh(run, &candidate, &options, &evidence, pending);
        if decided.is_none() {
            run.trace.semantic = SemanticUse::Rejected;
        }
        decided
    }

    /// What the model would be asked for `u` (benchmarks and diagnostics):
    /// the request and the trusted application behind each option.
    pub fn semantic_request(
        &self,
        u: &Utterance<'_>,
        dialogue: &Dialogue,
        now: u64,
    ) -> Option<(SemanticRequest, Vec<ApplicationSummary>)> {
        let run = Run {
            u,
            tokens: grammar::tokens(u.text),
            trace: UnderstandingTrace {
                normalized: String::new(),
                tier: ResolutionTier::None,
                candidates: Vec::new(),
                confidence: 0.0,
                semantic: SemanticUse::NotNeeded,
                semantic_ms: None,
                understanding_ms: 0.0,
            },
            understood: None,
        };
        let pending = dialogue.pending(now).map(|p| &p.clarification);
        self.build_request(&run, pending, dialogue, now)
            .map(|(request, options, _)| (request, options))
    }

    fn build_request(
        &self,
        run: &Run<'_, '_>,
        pending: Option<&Clarification>,
        dialogue: &Dialogue,
        now: u64,
    ) -> Option<(SemanticRequest, Vec<ApplicationSummary>, Vec<f32>)> {
        let apps = self.apps.as_ref()?;
        let (options, evidence) = self.options(run, pending, apps.as_ref());
        if options.is_empty() && pending.is_none() {
            return None;
        }
        let offered: Vec<usize> =
            pending.map_or_else(Vec::new, |p| (0..p.candidates.len()).collect());
        let request = SemanticRequest {
            text: run.u.text.to_owned(),
            language: run.u.language.map(str::to_owned),
            options: options
                .iter()
                .map(|a| SemanticOption {
                    name: a.display_name.clone(),
                })
                .collect(),
            pending: pending.map(|p| PendingQuestion {
                action: p.action,
                offered,
            }),
            previous: dialogue
                .recent(now)
                .filter_map(|t| t.summary.clone())
                .collect(),
        };
        Some((request, options, evidence))
    }

    /// Trusted candidates for the model: the pending question's options
    /// first, then the catalog entries most similar to any short phrase of
    /// the utterance.
    fn options(
        &self,
        run: &Run<'_, '_>,
        pending: Option<&Clarification>,
        apps: &dyn ApplicationDirectory,
    ) -> (Vec<ApplicationSummary>, Vec<f32>) {
        let mut options: Vec<ApplicationSummary> = Vec::new();
        let mut evidence: Vec<f32> = Vec::new();
        if let Some(p) = pending {
            for c in &p.candidates {
                options.push(c.clone());
                evidence.push(1.0);
            }
        }
        let start = grammar::skip_fillers(&run.tokens);
        let keys: Vec<&str> = run.tokens[start..].iter().map(|t| t.key.as_str()).collect();
        let mut phrases: Vec<String> = Vec::new();
        for n in 1..=MAX_NGRAM.min(keys.len()) {
            for window in keys.windows(n) {
                let phrase = window.join(" ");
                if phrase.chars().count() >= 3 {
                    phrases.push(phrase);
                }
            }
        }
        let mut best: Vec<(ApplicationSummary, f32)> = apps
            .names()
            .into_iter()
            .filter_map(|entry| {
                let score = phrases
                    .iter()
                    .flat_map(|p| entry.names.iter().map(move |n| similarity(p, n).combined))
                    .fold(0.0f32, f32::max);
                (score >= OPTION_MIN).then_some((entry.application, score))
            })
            .collect();
        best.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (app, score) in best {
            if options.len() >= semantic::MAX_OPTIONS {
                break;
            }
            if !options.iter().any(|o| o.id == app.id) {
                options.push(app);
                evidence.push(score);
            }
        }
        (options, evidence)
    }

    /// The deterministic confidence policy applied to a model answer.
    fn weigh(
        &self,
        run: &mut Run<'_, '_>,
        candidate: &SemanticCandidate,
        options: &[ApplicationSummary],
        evidence: &[f32],
        pending: Option<&Clarification>,
    ) -> Option<Intent> {
        let model = candidate.confidence;
        let chosen = candidate
            .target
            .and_then(|i| options.get(i).map(|a| (i, a)));
        // The command word must match the action: a model reading "close
        // Excel" as "open Excel" never acts.
        let heard = |verbs: &[&str], common: &[&str]| {
            run.tokens
                .iter()
                .any(|t| verb_likeness(&t.key, verbs, common) >= policy::VERB_LIKE)
        };
        let open_heard = heard(grammar::OPEN_VERBS, OPEN_LIKE);
        let close_heard = heard(grammar::CLOSE_VERBS, CLOSE_LIKE);
        run.trace.tier = ResolutionTier::Semantic;
        match (candidate.intent, chosen) {
            (SemanticIntent::ClarificationAnswer, Some((i, app))) => {
                // Only an option SERSHI offered may be chosen.
                let question = pending?;
                if i >= question.candidates.len() || model < policy::SEMANTIC_MIN_MODEL {
                    return None;
                }
                run.trace.confidence = model;
                Some(app_call(question.action, app, CallOrigin::Agent))
            }
            (
                SemanticIntent::OpenApplication | SemanticIntent::CloseApplication,
                Some((i, app)),
            ) => {
                let action = if candidate.intent == SemanticIntent::OpenApplication {
                    AppAction::Open
                } else {
                    AppAction::Close
                };
                let verb_heard = match action {
                    AppAction::Open => open_heard && !close_heard,
                    AppAction::Close => close_heard,
                };
                let score = policy::semantic_score(
                    evidence.get(i).copied().unwrap_or(0.0),
                    model,
                    verb_heard,
                    run.u.asr_confidence,
                );
                run.trace.confidence = score.max(0.0);
                match policy::decide_semantic(action, score, verb_heard) {
                    SemanticDecision::Act if !candidate.needs_clarification => {
                        run.understood = Some(UnderstoodAs {
                            action,
                            application: app.clone(),
                        });
                        Some(app_call(action, app, CallOrigin::Agent))
                    }
                    SemanticDecision::Act | SemanticDecision::Ask => Some(clarify(
                        ClarificationKind::DidYouMean,
                        action,
                        vec![app.clone()],
                    )),
                    SemanticDecision::Nothing => None,
                }
            }
            (SemanticIntent::OpenApplication | SemanticIntent::CloseApplication, None)
                if model >= policy::SEMANTIC_MIN_MODEL && (open_heard || close_heard) =>
            {
                let action = if candidate.intent == SemanticIntent::OpenApplication {
                    AppAction::Open
                } else {
                    AppAction::Close
                };
                Some(clarify(ClarificationKind::WhichApplication, action, vec![]))
            }
            // Memory questions are answered by keywords; a model-only
            // "system_memory" (the 0.6B model read "Shut Excel down" so) is
            // not acted on.
            _ => None,
        }
    }
}

/// Per-request scratch state.
struct Run<'u, 't> {
    u: &'u Utterance<'t>,
    tokens: Vec<Token<'t>>,
    trace: UnderstandingTrace,
    understood: Option<UnderstoodAs>,
}

impl Run<'_, '_> {
    fn note_candidates(&mut self, ranked: &[Scored]) {
        self.trace.candidates = ranked
            .iter()
            .take(3)
            .map(|s| CandidateScore {
                application: s.application.clone(),
                score: s.similarity.combined,
            })
            .collect();
    }
}

enum Target {
    Act(Intent),
    Ask(Intent),
    Unresolved,
    NoCatalog,
}

fn is_app_tool(call: &ToolCall) -> bool {
    [OPEN_APPLICATION, CLOSE_APPLICATION].contains(&call.tool_id.as_str())
}

fn fuzzy_tier(best: &Scored) -> ResolutionTier {
    if best.similarity.phonetic > best.similarity.lexical {
        ResolutionTier::Phonetic
    } else {
        ResolutionTier::Fuzzy
    }
}

/// Catalog entries ranked by their best-matching name (best first).
fn rank(names: &[CatalogNames], query: &str) -> Vec<Scored> {
    let mut ranked: Vec<Scored> = names
        .iter()
        .filter_map(|entry| {
            entry
                .names
                .iter()
                .filter(|n| n.chars().count() >= 3)
                .map(|n| similarity(query, n))
                .max_by(|a, b| a.combined.total_cmp(&b.combined))
                .map(|similarity| Scored {
                    application: entry.application.clone(),
                    similarity,
                })
        })
        .collect();
    ranked.sort_by(|a, b| b.similarity.combined.total_cmp(&a.similarity.combined));
    ranked.truncate(5);
    ranked
}

/// Common command words a misheard word is compared with. Short or rare
/// verbs ("run", "rode") are recognised only when spelled exactly: "code"
/// must not look like the Portuguese "rode".
const OPEN_LIKE: &[&str] = &[
    "abrir", "abre", "abra", "open", "launch", "iniciar", "inicia", "ejecutar", "executar",
    "ponme", "abreme",
];
const CLOSE_LIKE: &[&str] = &["cerrar", "cierra", "close", "fechar", "feche", "encerrar"];

/// How much a word looks like a command verb (0–1): 1 for any exact verb,
/// otherwise the best similarity to a common one.
fn verb_likeness(word: &str, verbs: &[&str], common: &[&str]) -> f32 {
    if verbs.contains(&word) {
        return 1.0;
    }
    common
        .iter()
        .map(|v| similarity(word, v).combined)
        .fold(0.0, f32::max)
}

fn target_has_verb(target: &str) -> bool {
    grammar::tokens(target).iter().any(|t| {
        grammar::OPEN_VERBS.contains(&t.key.as_str())
            || grammar::CLOSE_VERBS.contains(&t.key.as_str())
    })
}

/// Splits "Outlook, Google Chrome" / "Outlook y Chrome" into names.
fn split_names(text: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    for part in text.split([',', ';', '&', '+']) {
        let toks = grammar::tokens(part);
        let mut current: Vec<&str> = Vec::new();
        for t in &toks {
            if matches!(t.key.as_str(), "y" | "and" | "e" | "o" | "or" | "ou")
                && !current.is_empty()
            {
                pieces.push(current.join(" "));
                current.clear();
            } else {
                current.push(t.raw);
            }
        }
        if !current.is_empty() {
            pieces.push(current.join(" "));
        }
    }
    pieces
        .into_iter()
        .map(|p| grammar::target(&grammar::tokens(&p)))
        .filter(|p| !p.is_empty())
        .collect()
}

fn clarify(
    kind: ClarificationKind,
    action: AppAction,
    candidates: Vec<ApplicationSummary>,
) -> Intent {
    Intent::Clarify(Clarification {
        kind,
        action,
        candidates,
    })
}

fn tool_for(action: AppAction) -> &'static str {
    match action {
        AppAction::Open => OPEN_APPLICATION,
        AppAction::Close => CLOSE_APPLICATION,
    }
}

/// A call naming a trusted application by its catalog id. The tool
/// resolves the id through the catalog again; nothing else is passed.
fn app_call(action: AppAction, app: &ApplicationSummary, origin: CallOrigin) -> Intent {
    match ToolId::new(tool_for(action)) {
        Ok(id) => Intent::UseTool(ToolCall::new(id, json!({ "application": app.id }), origin)),
        Err(_) => Intent::NotUnderstood,
    }
}

/// A call with the user's own words when no catalog is available or the
/// name matched nothing (the tool then reports "not found").
fn raw_call(action: AppAction, target: &str) -> Intent {
    match ToolId::new(tool_for(action)) {
        Ok(id) => Intent::UseTool(ToolCall::new(
            id,
            json!({ "application": target }),
            CallOrigin::User,
        )),
        Err(_) => Intent::NotUnderstood,
    }
}
