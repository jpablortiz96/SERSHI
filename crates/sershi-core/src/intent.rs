//! Intent understanding: turning a user's words into a structured decision.
//!
//! The resolver's output is always data — a [`ToolCall`] naming a registered
//! tool, a reply, or an honest "not yet". It never produces commands. When a
//! language model arrives (v0.1 LLM milestone) it implements
//! [`IntentResolver`] and its tool calls flow through the exact same executor
//! and policy as the rules below; the only difference is their
//! [`CallOrigin::Agent`] origin.

use std::fmt::Debug;

use serde_json::Value;

use crate::ids::ToolId;
use crate::tool::{CallOrigin, ToolCall};

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// Run a tool.
    UseTool(ToolCall),
    /// Answer directly without acting.
    Reply(String),
    /// Understood, but the capability does not exist yet.
    NotYetAvailable {
        capability: &'static str,
        milestone: &'static str,
    },
    /// Not understood.
    NotUnderstood,
}

pub trait IntentResolver: Send + Sync + Debug {
    fn resolve(&self, text: &str) -> Intent;
}

/// Deterministic keyword resolver used until a language model is configured.
/// Intentionally small: it exists to prove the pipeline, not to fake
/// understanding.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeywordIntentResolver;

const GREETING: &str = "Hello. I'm SERSHI. I can tell you about this computer's system, \
memory and processor. Language understanding arrives once an AI provider is connected.";

const HELP: &str = "Right now I can report system information, memory usage and processor \
load. Try \"How much memory am I using?\". Opening apps, files, voice and connected \
services are on the roadmap.";

impl IntentResolver for KeywordIntentResolver {
    fn resolve(&self, text: &str) -> Intent {
        let lowered = text.to_lowercase();
        let words: Vec<&str> = lowered
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let has = |candidates: &[&str]| words.iter().any(|w| candidates.contains(w));

        if words.is_empty() {
            return Intent::NotUnderstood;
        }
        // Action verbs first: "open task manager to check memory" is an
        // action request, not a memory question.
        if matches!(
            words[0],
            "open" | "launch" | "start" | "run" | "close" | "quit"
        ) {
            return Intent::NotYetAvailable {
                capability: "Opening and closing applications",
                milestone: "v0.1",
            };
        }
        if has(&["ram", "memory"]) {
            return tool("system.get_memory");
        }
        if has(&["cpu", "processor", "load"]) {
            return tool("system.get_cpu");
        }
        if has(&["specs", "system", "computer", "machine", "pc", "os"]) {
            return tool("system.get_info");
        }
        if has(&["battery", "charge", "charging"]) {
            return Intent::NotYetAvailable {
                capability: "Battery status",
                milestone: "v0.1",
            };
        }
        if has(&["file", "files", "folder", "clipboard", "screen", "copied"]) {
            return Intent::NotYetAvailable {
                capability: "Files, clipboard and screen context",
                milestone: "v0.2",
            };
        }
        if has(&["email", "mail", "calendar", "meeting", "meetings"]) {
            return Intent::NotYetAvailable {
                capability: "Email and calendar",
                milestone: "v0.4",
            };
        }
        if has(&["hello", "hi", "hey", "hola"]) {
            return Intent::Reply(GREETING.to_owned());
        }
        if has(&["help"]) || lowered.contains("what can you do") {
            return Intent::Reply(HELP.to_owned());
        }
        Intent::NotUnderstood
    }
}

fn tool(id: &'static str) -> Intent {
    match ToolId::new(id) {
        Ok(tool_id) => Intent::UseTool(ToolCall::new(tool_id, Value::Null, CallOrigin::User)),
        Err(_) => Intent::NotUnderstood,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_of(text: &str) -> Option<String> {
        match KeywordIntentResolver.resolve(text) {
            Intent::UseTool(call) => Some(call.tool_id.to_string()),
            _ => None,
        }
    }

    #[test]
    fn maps_system_questions_to_read_only_tools() {
        assert_eq!(
            tool_of("How much RAM am I using?").as_deref(),
            Some("system.get_memory")
        );
        assert_eq!(tool_of("cpu load").as_deref(), Some("system.get_cpu"));
        assert_eq!(
            tool_of("Tell me about this computer").as_deref(),
            Some("system.get_info")
        );
    }

    #[test]
    fn action_requests_are_honestly_unavailable() {
        for text in [
            "Open Spotify",
            "run the tests",
            "open task manager to check memory",
        ] {
            assert!(
                matches!(
                    KeywordIntentResolver.resolve(text),
                    Intent::NotYetAvailable { .. }
                ),
                "{text}"
            );
        }
    }

    #[test]
    fn rule_based_calls_are_user_originated_and_carry_no_input() {
        let Intent::UseTool(call) = KeywordIntentResolver.resolve("memory") else {
            panic!("expected a tool call");
        };
        assert_eq!(call.origin, CallOrigin::User);
        assert_eq!(call.input, Value::Null);
    }

    #[test]
    fn unknown_text_is_not_guessed() {
        assert_eq!(
            KeywordIntentResolver.resolve("rm -rf /"),
            Intent::NotUnderstood
        );
        assert_eq!(KeywordIntentResolver.resolve("   "), Intent::NotUnderstood);
    }
}
