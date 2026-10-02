//! The Agent Brain (Prompt 4): conversation, follow-ups and bounded plans.
//!
//! > The brain reasons. The brain does not execute.
//!
//! [`AgentBrainPort`] is the provider-neutral domain port: it receives a
//! [`BrainRequest`] (the request text, the response language, a bounded
//! session context, trusted applications under opaque handles and the
//! capability manifest generated from the tool registry) and returns a
//! [`BrainDecision`]: an answer, a question, tool requests, a cancellation
//! or "unknown". [`LocalAgentBrain`] implements it over any
//! [`TextGenerator`] (the local llama.cpp engine); a future cloud provider
//! would implement the same port without touching the core.
//!
//! What the model can never do, by construction:
//!
//! - name a tool outside the manifest (the grammar and parser reject it);
//! - name an application outside the offered handles (no names, ids or
//!   paths are accepted, and it never sees a path);
//! - produce more than [`MAX_PLAN_STEPS`] steps;
//! - run anything: decisions are validated by the service, steps become
//!   `ToolCall`s with `CallOrigin::Agent` and each one goes through the
//!   executor, policy and — when required — the trusted confirmation
//!   window, separately;
//! - approve anything: no decision kind, field or tool can.
//!
//! Its reasoning is never requested, stored, logged or shown: only the
//! structured decision and a short user-facing message exist.

pub mod context;
pub mod contract;
pub mod prompt;
pub mod route;

use std::fmt::Debug;

use thiserror::Error;

use crate::platform::{CapabilityStatus, PlatformCapability};
use crate::tool::ToolDefinition;
use crate::understanding::semantic::{ChatTemplate, Generation, RouterError, TextGenerator};

pub use context::{ActiveApp, AppEvent, SessionContext, SystemFact};
pub use contract::{BRAIN_PROMPT_VERSION, BrainDecision};

/// Most tool steps in one plan. Five covers the corpus' longest real
/// request ("open three apps, then tell me the memory") with room to spare;
/// longer requests are asked to be split (docs/AGENT_BRAIN.md).
pub const MAX_PLAN_STEPS: usize = 5;
/// Most tokens one decision may take.
pub const MAX_OUTPUT_TOKENS: u32 = 220;

/// One tool the brain may request, generated from the tool registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    /// The name the model uses (`open_application`).
    pub name: &'static str,
    /// The registry tool it maps to (`system.open_application`).
    pub tool_id: &'static str,
    /// Whether it takes one application handle.
    pub takes_app: bool,
    /// One line for the model.
    pub description: &'static str,
}

/// The brain vocabulary for registry tools. Only tools present in the
/// registry are offered, so the manifest cannot drift from what exists;
/// there is deliberately no entry for anything resembling a shell, a
/// process, a path or a script.
const VOCABULARY: [Capability; 5] = [
    Capability {
        name: "open_application",
        tool_id: "system.open_application",
        takes_app: true,
        description: "open one installed application",
    },
    Capability {
        name: "close_application",
        tool_id: "system.close_application",
        takes_app: true,
        description: "ask one application to close (the user approves it in a confirmation window)",
    },
    Capability {
        name: "get_memory",
        tool_id: "system.get_memory",
        takes_app: false,
        description: "read how much memory (RAM) is in use",
    },
    Capability {
        name: "get_cpu",
        tool_id: "system.get_cpu",
        takes_app: false,
        description: "read the processor load",
    },
    Capability {
        name: "get_system_info",
        tool_id: "system.get_info",
        takes_app: false,
        description: "read this computer's operating system, processor and memory size",
    },
];

/// Every capability the brain vocabulary knows (tests and benchmarks; the
/// product uses [`manifest`], which keeps only registered tools).
pub const VOCABULARY_FOR_TESTS: [Capability; 5] = VOCABULARY;

/// The capability manifest for the tools actually registered.
pub fn manifest<'a>(definitions: impl IntoIterator<Item = &'a ToolDefinition>) -> Vec<Capability> {
    let registered: Vec<String> = definitions
        .into_iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    VOCABULARY
        .into_iter()
        .filter(|c| registered.iter().any(|id| id == c.tool_id))
        .collect()
}

/// What SERSHI will not do at all (product boundaries, not roadmap items).
const BOUNDARIES: [&str; 3] = [
    "browsing the web or reading current news",
    "running commands, scripts or programs by path",
    "changing system settings",
];

/// What the brain must never promise: capabilities the platform report
/// marks planned or unsupported, plus SERSHI's fixed boundaries.
pub fn unavailable(report: &[PlatformCapability]) -> Vec<String> {
    report
        .iter()
        .filter(|c| {
            matches!(
                c.status,
                CapabilityStatus::Planned | CapabilityStatus::Unsupported
            )
        })
        .map(|c| c.label.to_lowercase())
        .chain(BOUNDARIES.iter().map(|b| (*b).to_owned()))
        .collect()
}

/// One validated step: a capability and, when it takes one, the index of
/// an offered application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrainStep {
    pub capability: &'static str,
    pub app: Option<usize>,
}

/// Everything the brain sees. No paths, ids, confirmation data, secrets or
/// tool internals: names, handles and short structured context only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainRequest {
    /// The request, sanitized and bounded (data, never instructions).
    pub text: String,
    /// Language to write `message` in (`es-419`, `en-US`, `pt-BR`).
    pub response_language: String,
    /// Offered applications, by display name; handle `aN` is index N-1.
    pub apps: Vec<String>,
    /// Recent turns and active entities, already written as short lines
    /// that refer to applications by handle.
    pub context: Vec<String>,
    /// Tools the brain may request.
    pub capabilities: Vec<Capability>,
    /// What SERSHI cannot do yet (so it never promises it).
    pub unavailable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BrainError {
    #[error("no local brain model is installed")]
    NotInstalled,
    #[error("the local brain is unavailable")]
    Unavailable,
    #[error("the local brain did not answer in time")]
    Timeout,
    #[error("brain inference failed")]
    Failed,
    #[error("the brain's output was rejected")]
    InvalidOutput,
}

impl From<RouterError> for BrainError {
    fn from(e: RouterError) -> Self {
        match e {
            RouterError::NotInstalled => Self::NotInstalled,
            RouterError::Unavailable => Self::Unavailable,
            RouterError::Timeout => Self::Timeout,
            RouterError::Failed => Self::Failed,
            RouterError::InvalidOutput => Self::InvalidOutput,
        }
    }
}

/// The provider-neutral domain port.
pub trait AgentBrainPort: Send + Sync + Debug {
    fn decide(&self, request: &BrainRequest) -> Result<BrainDecision, BrainError>;

    /// A hint that a request may come soon (load while the user speaks).
    fn prepare(&self) {}
}

/// [`AgentBrainPort`] over a local [`TextGenerator`].
#[derive(Debug)]
pub struct LocalAgentBrain<G> {
    generator: G,
    template: ChatTemplate,
}

impl<G: TextGenerator> LocalAgentBrain<G> {
    pub fn new(generator: G, template: ChatTemplate) -> Self {
        Self {
            generator,
            template,
        }
    }
}

impl<G: TextGenerator> AgentBrainPort for LocalAgentBrain<G> {
    fn decide(&self, request: &BrainRequest) -> Result<BrainDecision, BrainError> {
        let generation = Generation {
            prompt: prompt::render(request, self.template),
            grammar: contract::grammar(&request.capabilities, request.apps.len()),
            max_tokens: MAX_OUTPUT_TOKENS,
        };
        let text = self.generator.generate(&generation)?;
        contract::parse(&text, &request.capabilities, request.apps.len())
    }

    fn prepare(&self) {
        self.generator.prepare();
    }
}
