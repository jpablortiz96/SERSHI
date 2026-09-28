//! Intent understanding: turning a user's words into a structured decision.
//!
//! The resolver's output is always data — a [`ToolCall`] naming a registered
//! tool, a reply, or an honest "not yet". It never produces commands. When a
//! language model arrives (v0.1 LLM milestone) it implements
//! [`IntentResolver`] and its tool calls flow through the exact same executor
//! and policy as the rules below; the only difference is their
//! [`CallOrigin::Agent`] origin.

use std::fmt::Debug;

use serde::Serialize;
use serde_json::Value;

use crate::ids::ToolId;
use crate::tool::{CallOrigin, ToolCall};

/// A fixed answer SERSHI can give without acting. The UI renders it in the
/// user's interface language; [`AnswerTopic::canonical_text`] is the English
/// fallback carried in `CommandOutcome::reply`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AnswerTopic {
    Greeting,
    Help,
}

impl AnswerTopic {
    pub fn canonical_text(self) -> &'static str {
        match self {
            Self::Greeting => {
                "Hello. I'm SERSHI. I can tell you about this computer's system, memory and \
                 processor. Language understanding arrives once an AI provider is connected."
            }
            Self::Help => {
                "Right now I can report system information, memory usage and processor load. \
                 Try \"How much memory am I using?\". Opening apps, files, voice and connected \
                 services are on the roadmap."
            }
        }
    }
}

/// A capability SERSHI understood but does not have yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnavailableCapability {
    /// Stable id, shared with the platform capability report where one exists.
    pub id: &'static str,
    /// Canonical English label, used for the English reply and activity.
    pub label: &'static str,
    pub milestone: &'static str,
}

const APPS: UnavailableCapability = UnavailableCapability {
    id: "apps.launch",
    label: "Opening and closing applications",
    milestone: "v0.1",
};
const BATTERY: UnavailableCapability = UnavailableCapability {
    id: "system.battery",
    label: "Battery status",
    milestone: "v0.1",
};
const CONTEXT: UnavailableCapability = UnavailableCapability {
    id: "context.files",
    label: "Files, clipboard and screen context",
    milestone: "v0.2",
};
const CONNECTED: UnavailableCapability = UnavailableCapability {
    id: "connected.mail_calendar",
    label: "Email and calendar",
    milestone: "v0.4",
};

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// Run a tool.
    UseTool(ToolCall),
    /// Answer directly without acting.
    Answer(AnswerTopic),
    /// Understood, but the capability does not exist yet.
    NotYetAvailable(UnavailableCapability),
    /// Not understood.
    NotUnderstood,
}

pub trait IntentResolver: Send + Sync + Debug {
    fn resolve(&self, text: &str) -> Intent;
}

/// Deterministic keyword resolver used until a language model is configured.
/// Intentionally small: it exists to prove the pipeline, not to fake
/// understanding. It recognises English, Spanish and Portuguese keywords so
/// the suggestions shown in every supported interface language work; this is
/// vocabulary matching, not language detection.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeywordIntentResolver;

/// Leading verbs that request an action on an application.
const ACTION_VERBS: &[&str] = &[
    // English
    "open", "launch", "start", "run", "close", "quit", // Spanish
    "abre", "abrir", "abra", "inicia", "iniciar", "ejecuta", "ejecutar", "cierra", "cerrar",
    // Portuguese
    "inicie", "execute", "executar", "rode", "feche", "fechar",
];
const MEMORY: &[&str] = &["ram", "memory", "memoria", "memória"];
const CPU: &[&str] = &["cpu", "processor", "load", "procesador", "processador"];
const SYSTEM: &[&str] = &[
    "specs",
    "system",
    "computer",
    "machine",
    "pc",
    "os",
    "sistema",
    "computadora",
    "computador",
    "equipo",
    "máquina",
    "maquina",
];
const BATTERY_WORDS: &[&str] = &["battery", "charge", "charging", "batería", "bateria"];
const CONTEXT_WORDS: &[&str] = &[
    "file",
    "files",
    "folder",
    "clipboard",
    "screen",
    "copied",
    "archivo",
    "archivos",
    "carpeta",
    "portapapeles",
    "pantalla",
    "copié",
    "arquivo",
    "arquivos",
    "pasta",
    "tela",
    "copiei",
];
const CONNECTED_WORDS: &[&str] = &[
    "email",
    "mail",
    "calendar",
    "meeting",
    "meetings",
    "correo",
    "calendario",
    "reunión",
    "reuniones",
    "calendário",
    "reunião",
    "reuniões",
];
const GREETINGS: &[&str] = &["hello", "hi", "hey", "hola", "olá", "ola", "oi"];
const HELP_WORDS: &[&str] = &["help", "ayuda", "ajuda"];

impl IntentResolver for KeywordIntentResolver {
    fn resolve(&self, text: &str) -> Intent {
        let lowered = text.to_lowercase();
        let words: Vec<&str> = lowered
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let has = |candidates: &[&str]| words.iter().any(|w| candidates.contains(w));

        let Some(first) = words.first() else {
            return Intent::NotUnderstood;
        };
        // Action verbs first: "open task manager to check memory" is an
        // action request, not a memory question.
        if ACTION_VERBS.contains(first) {
            return Intent::NotYetAvailable(APPS);
        }
        if has(MEMORY) {
            return tool("system.get_memory");
        }
        if has(CPU) {
            return tool("system.get_cpu");
        }
        if has(SYSTEM) {
            return tool("system.get_info");
        }
        if has(BATTERY_WORDS) {
            return Intent::NotYetAvailable(BATTERY);
        }
        if has(CONTEXT_WORDS) {
            return Intent::NotYetAvailable(CONTEXT);
        }
        if has(CONNECTED_WORDS) {
            return Intent::NotYetAvailable(CONNECTED);
        }
        if has(GREETINGS) {
            return Intent::Answer(AnswerTopic::Greeting);
        }
        if has(HELP_WORDS) || lowered.contains("what can you do") {
            return Intent::Answer(AnswerTopic::Help);
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
                    Intent::NotYetAvailable(_)
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
    fn understands_the_suggestions_of_every_interface_language() {
        for (text, expected) in [
            ("¿Cuánta memoria estoy usando?", "system.get_memory"),
            ("¿Cuál es la carga del procesador?", "system.get_cpu"),
            ("Háblame de esta computadora", "system.get_info"),
            ("Quanta memória estou usando?", "system.get_memory"),
            ("Qual é a carga do processador?", "system.get_cpu"),
            ("Fale sobre este computador", "system.get_info"),
        ] {
            assert_eq!(tool_of(text).as_deref(), Some(expected), "{text}");
        }
        for text in ["Abre Spotify", "Abra o Spotify"] {
            assert_eq!(
                KeywordIntentResolver.resolve(text),
                Intent::NotYetAvailable(APPS),
                "{text}"
            );
        }
        assert_eq!(
            KeywordIntentResolver.resolve("Hola"),
            Intent::Answer(AnswerTopic::Greeting)
        );
        assert_eq!(
            KeywordIntentResolver.resolve("ajuda"),
            Intent::Answer(AnswerTopic::Help)
        );
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
