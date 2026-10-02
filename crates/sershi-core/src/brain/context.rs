//! Session context for conversation (Prompt 4): what SERSHI and the user
//! just did together, so "ciérralo", "¿y el procesador?" or "now PowerPoint"
//! make sense.
//!
//! - **Memory only.** Never written to disk, logged or sent anywhere but the
//!   local brain. Gone when SERSHI exits or the user starts a new
//!   conversation ([`SessionContext::reset`]).
//! - **Bounded.** At most [`MAX_APPS`] applications and one system fact.
//! - **Short-lived.** An application stays a valid referent for
//!   [`ENTITY_TTL_MS`]; after that "close it" is not understood rather than
//!   resolved against something the user has forgotten.
//! - **Trusted entries only.** Applications are recorded from catalog
//!   entries that tools actually acted on; facts from tool outputs. The
//!   user's words never become an entity.
//!
//! Context resolves meaning; it never authorizes. A resolved "close it" is
//! an ordinary `close_application` call: policy and the trusted
//! confirmation decide.

use std::collections::VecDeque;

use crate::apps::ApplicationSummary;

/// How long an application or fact stays a valid referent.
pub const ENTITY_TTL_MS: u64 = 5 * 60_000;
/// Most recent applications kept.
pub const MAX_APPS: usize = 4;

/// What happened to an application in this conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    Opened,
    /// A close was requested (confirmation shown, or completed).
    CloseRequested,
    /// Named in a question or answer, without acting.
    Mentioned,
}

impl AppEvent {
    fn verb(self) -> &'static str {
        match self {
            Self::Opened => "opened",
            Self::CloseRequested => "asked to close",
            Self::Mentioned => "mentioned",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveApp {
    pub app: ApplicationSummary,
    pub event: AppEvent,
    pub at: u64,
    /// The request (turn) that touched it: applications touched by one
    /// request ("abre Chrome y Outlook") are referred to together.
    pub request: u64,
}

/// The last system fact a tool returned (trusted numbers, no text).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemFact {
    Memory { used_bytes: u64, total_bytes: u64 },
    Cpu { usage_percent: Option<f32> },
    Info,
}

impl SystemFact {
    fn line(&self) -> String {
        const GB: f64 = 1_000_000_000.0;
        match *self {
            Self::Memory {
                used_bytes,
                total_bytes,
            } => format!(
                "memory checked: {:.1} GB used of {:.1} GB",
                used_bytes as f64 / GB,
                total_bytes as f64 / GB
            ),
            Self::Cpu {
                usage_percent: Some(p),
            } => format!("processor checked: {p:.0}% load"),
            Self::Cpu {
                usage_percent: None,
            } => "processor checked".to_owned(),
            Self::Info => "system information shown".to_owned(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SessionContext {
    apps: VecDeque<ActiveApp>,
    fact: Option<(u64, SystemFact)>,
}

impl SessionContext {
    /// Records an application a tool acted on (newest first, deduplicated).
    pub fn record_app(&mut self, app: ApplicationSummary, event: AppEvent, now: u64, request: u64) {
        self.apps.retain(|a| a.app.id != app.id);
        self.apps.push_front(ActiveApp {
            app,
            event,
            at: now,
            request,
        });
        self.apps.truncate(MAX_APPS);
    }

    pub fn record_fact(&mut self, fact: SystemFact, now: u64) {
        self.fact = Some((now, fact));
    }

    /// Unexpired applications, newest first.
    pub fn apps(&self, now: u64) -> impl Iterator<Item = &ActiveApp> {
        self.apps
            .iter()
            .filter(move |a| now.saturating_sub(a.at) < ENTITY_TTL_MS)
    }

    /// The single application a reference ("it", "lo", "ele") can mean:
    /// the most recent one, unless the same request touched another one too
    /// ("abre Chrome y Outlook" → "ciérralo" is ambiguous and SERSHI asks).
    /// Separate turns are not ambiguous however quickly they follow each
    /// other: in a voice session "abre Word" right after closing Excel
    /// makes Word "it" (Gate 4.1).
    pub fn referent(&self, now: u64) -> Referent<'_> {
        let recent: Vec<&ActiveApp> = self.apps(now).collect();
        match recent.as_slice() {
            [] => Referent::None,
            [only] => Referent::One(only),
            [first, second, ..] => {
                // Touched together (one plan, or one request): ambiguous.
                if first.request == second.request {
                    Referent::Ambiguous(
                        recent
                            .iter()
                            .take_while(|a| a.request == first.request)
                            .copied()
                            .collect(),
                    )
                } else {
                    Referent::One(first)
                }
            }
        }
    }

    pub fn fact(&self, now: u64) -> Option<SystemFact> {
        self.fact
            .filter(|(at, _)| now.saturating_sub(*at) < ENTITY_TTL_MS)
            .map(|(_, f)| f)
    }

    /// Short lines for the brain, naming applications by their offered
    /// handle (`handle(app)` returns it, or `None` if it was not offered).
    pub fn lines(
        &self,
        now: u64,
        handle: impl Fn(&ApplicationSummary) -> Option<String>,
    ) -> Vec<String> {
        let mut lines: Vec<String> = self
            .apps(now)
            .filter_map(|a| {
                let h = handle(&a.app)?;
                let ago = now.saturating_sub(a.at) / 1000;
                Some(format!(
                    "{} {h} ({}) {ago} s ago",
                    a.event.verb(),
                    a.app.display_name
                ))
            })
            .collect();
        if let Some(fact) = self.fact(now) {
            lines.push(fact.line());
        }
        lines
    }

    /// Forgets everything ("new conversation", session end).
    pub fn reset(&mut self) {
        self.apps.clear();
        self.fact = None;
    }
}

/// What a reference can mean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Referent<'a> {
    None,
    One(&'a ActiveApp),
    Ambiguous(Vec<&'a ActiveApp>),
}
