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
use crate::understanding::Clarification;

/// A fixed answer SERSHI can give without acting. The UI renders it in the
/// user's interface language; [`AnswerTopic::canonical_text`] is the English
/// fallback carried in `CommandOutcome::reply`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AnswerTopic {
    Greeting,
    Help,
    /// A negated request ("No abras Chrome"): acknowledged, nothing done.
    NoAction,
    /// A request to run a command, script or program by path: SERSHI
    /// never does that (Prompt 4); it says so and does nothing.
    NoCommands,
}

impl AnswerTopic {
    pub fn canonical_text(self) -> &'static str {
        match self {
            Self::Greeting => {
                "Hello. I'm SERSHI. I can tell you about this computer's system, memory and \
                 processor. Language understanding arrives once an AI provider is connected."
            }
            Self::Help => {
                "I can report system information, memory usage and processor load, and open or \
                 close installed applications — by typing or with the microphone. Try \"Open \
                 Notepad\". Files and connected services are on the roadmap."
            }
            Self::NoAction => "OK — I won't do anything.",
            Self::NoCommands => {
                "I can't run commands, scripts or programs by path. I can open installed \
                 applications and check memory and the processor."
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
    /// "Cancel": withdraw whatever is pending. Cancelling only ever reduces
    /// authority. There is deliberately no counterpart: no intent can
    /// approve anything (approval exists only on the trusted surface).
    Cancel,
    /// Ask the user instead of guessing (Gate 3C). The candidates are
    /// trusted catalog entries; the answer is resolved only among them.
    Clarify(Clarification),
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

/// Imperative verbs that open an application.
#[rustfmt::skip]
const OPEN_VERBS: &[&str] = &[
    /* en */ "open", "launch", "start", "run",
    /* es */ "abre", "abrir", "abra", "abres", "abras", "inicia", "iniciar", "ejecuta", "ejecutar",
    /* pt */ "inicie", "execute", "executar", "rode",
];
/// Imperative verbs that close an application.
#[rustfmt::skip]
const CLOSE_VERBS: &[&str] = &[
    /* en */ "close", "quit", "exit",
    /* es */ "cierra", "cerrar", "cierre", "cierras", "cierres",
    /* pt */ "feche", "fechar", "encerre", "encerrar",
];
/// Courtesy prefixes skipped before the verb.
const POLITE_PREFIXES: &[&[&str]] = &[
    &["please"],
    &["can", "you"],
    &["could", "you"],
    &["por", "favor"],
    &["puedes"],
    &["podrias"],
    &["podrías"],
    &["pode"],
    &["voce", "pode"],
    &["você", "pode"],
];
/// Articles dropped between the verb and the name ("abra o Spotify").
#[rustfmt::skip]
const ARTICLES: &[&str] = &[
    "the", "el", "la", "los", "las", "un", "una", "o", "a", "os", "as", "um", "uma",
];
/// Trailing words that are not part of the name ("open Spotify app please").
#[rustfmt::skip]
const TRAILING_FILLER: &[&str] = &[
    "app", "application", "aplicación", "aplicacion", "aplicativo", "programa", "please",
    "favor", "por",
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
/// Whole-utterance cancellations ("Cancel", "Cancelar", "Cancelar ação").
/// Matched exactly (after courtesy words), so "cancel my meeting" is not one.
const CANCEL_PHRASES: &[&[&str]] = &[
    &["cancel"],
    &["cancel", "that"],
    &["cancel", "it"],
    &["never", "mind"],
    &["nevermind"],
    &["cancelar"],
    &["cancela"],
    &["cancele"],
    &["cancelalo"],
    &["cancélalo"],
    &["cancela", "eso"],
    &["cancelar", "acción"],
    &["cancelar", "accion"],
    &["cancelar", "ação"],
    &["cancelar", "acao"],
    &["cancele", "isso"],
];
/// Words ignored around a cancellation ("SERSHI, cancel please").
const CANCEL_FILLER: &[&str] = &["sershi", "please", "por", "favor", "ok", "okay"];
const HELP_WORDS: &[&str] = &["help", "ayuda", "ajuda"];

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
        let core: Vec<&str> = words
            .iter()
            .copied()
            .filter(|w| !CANCEL_FILLER.contains(w))
            .collect();
        if CANCEL_PHRASES.contains(&core.as_slice()) {
            return Intent::Cancel;
        }
        // Application commands first: "open task manager to check memory" is
        // an action request, not a memory question.
        if let Some(intent) = application_command(text) {
            return intent;
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

/// Recognises "<verb> <application>" commands. Only imperative commands
/// qualify: the verb must be the first word (after an optional courtesy
/// prefix), so "I like Spotify" or "Spotify is open" never act.
fn application_command(text: &str) -> Option<Intent> {
    let key = |t: &str| {
        t.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut rest: &[&str] = &tokens;

    for prefix in POLITE_PREFIXES {
        if rest.len() > prefix.len() && rest.iter().zip(prefix.iter()).all(|(t, p)| key(t) == *p) {
            rest = &rest[prefix.len()..];
            break;
        }
    }
    let (verb, mut name) = rest.split_first()?;
    let verb = key(verb);
    let tool_id = if OPEN_VERBS.contains(&verb.as_str()) {
        crate::apps::tools::OPEN_APPLICATION
    } else if CLOSE_VERBS.contains(&verb.as_str()) {
        crate::apps::tools::CLOSE_APPLICATION
    } else {
        return None;
    };
    if let Some((first, tail)) = name.split_first()
        && !tail.is_empty()
        && ARTICLES.contains(&key(first).as_str())
    {
        name = tail;
    }
    while let Some((last, head)) = name.split_last() {
        if !head.is_empty() && TRAILING_FILLER.contains(&key(last).as_str()) {
            name = head;
        } else {
            break;
        }
    }
    let application = name
        .join(" ")
        .trim_matches(|c: char| c.is_whitespace() || ".,;!?¿¡\"'“”«»".contains(c))
        .to_owned();
    if application.is_empty() || (name.len() == 1 && ARTICLES.contains(&key(name[0]).as_str())) {
        return Some(Intent::NotUnderstood);
    }
    let tool_id = ToolId::new(tool_id).ok()?;
    Some(Intent::UseTool(ToolCall::new(
        tool_id,
        serde_json::json!({ "application": application }),
        CallOrigin::User,
    )))
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

    fn app_command(text: &str) -> Option<(String, String)> {
        match KeywordIntentResolver.resolve(text) {
            Intent::UseTool(call) => Some((
                call.tool_id.to_string(),
                call.input["application"].as_str()?.to_owned(),
            )),
            _ => None,
        }
    }

    #[test]
    fn open_commands_in_three_languages() {
        for (text, app) in [
            ("Open Spotify", "Spotify"),
            ("Launch Spotify", "Spotify"),
            ("Start Spotify", "Spotify"),
            ("open the calculator app", "calculator"),
            ("Please open Google Chrome", "Google Chrome"),
            ("Abre Spotify", "Spotify"),
            ("Abrir Spotify", "Spotify"),
            ("Inicia Spotify", "Spotify"),
            ("Abre el Bloc de notas", "Bloc de notas"),
            ("abre la calculadora", "calculadora"),
            ("¿Puedes abrir Spotify?", "Spotify"),
            ("Abra o Spotify", "Spotify"),
            ("Inicie o Spotify", "Spotify"),
            ("Abra a Calculadora", "Calculadora"),
            // Second-person forms, as speech recognition often hears them.
            ("¿Abres Spotify?", "Spotify"),
            ("Abras Spotify.", "Spotify"),
            ("Abra o Visual Studio Code, por favor", "Visual Studio Code"),
        ] {
            assert_eq!(
                app_command(text),
                Some(("system.open_application".into(), app.into())),
                "{text}"
            );
        }
    }

    #[test]
    fn close_commands_in_three_languages() {
        for (text, app) in [
            ("Close Spotify", "Spotify"),
            ("quit Chrome", "Chrome"),
            ("Cierra Spotify", "Spotify"),
            ("Cerrar Spotify", "Spotify"),
            ("Cierras Spotify.", "Spotify"),
            ("Feche o Spotify", "Spotify"),
            ("Fechar Spotify", "Spotify"),
        ] {
            assert_eq!(
                app_command(text),
                Some(("system.close_application".into(), app.into())),
                "{text}"
            );
        }
    }

    #[test]
    fn statements_about_applications_never_act() {
        for text in [
            "I like Spotify",
            "Spotify is open",
            "is Spotify open?",
            "opening Spotify later",
            "the app to open is Spotify",
            "Me gusta abrir Spotify",
            "O Spotify está aberto",
        ] {
            assert_eq!(app_command(text), None, "{text}");
        }
    }

    #[test]
    fn a_verb_without_an_application_is_not_understood() {
        for text in ["open", "Abre", "abra o", "close the"] {
            assert_eq!(
                KeywordIntentResolver.resolve(text),
                Intent::NotUnderstood,
                "{text}"
            );
        }
    }

    #[test]
    fn application_names_are_passed_as_data_not_commands() {
        // The name is forwarded verbatim; the tool rejects anything path- or
        // command-like, and nothing is ever executed as text.
        let Some((_, app)) = app_command("open cmd /c del *") else {
            panic!("expected a tool call");
        };
        assert_eq!(app, "cmd /c del *");
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
    fn cancellation_is_an_exact_utterance() {
        for text in [
            "Cancel",
            "cancel that",
            "Cancelar.",
            "¡Cancela!",
            "Cancelar ação",
            "cancelar acción",
            "SERSHI, cancel please",
            "Cancelar, por favor",
            "Never mind",
        ] {
            assert_eq!(
                KeywordIntentResolver.resolve(text),
                Intent::Cancel,
                "{text}"
            );
        }
        for text in ["cancel my meeting", "how do I cancel", "cancelar Spotify"] {
            assert_ne!(
                KeywordIntentResolver.resolve(text),
                Intent::Cancel,
                "{text}"
            );
        }
    }

    #[test]
    fn approval_words_are_never_an_intent() {
        // Approval exists only on the trusted confirmation surface. Spoken or
        // typed, these are just words that SERSHI does not act on.
        for text in [
            "yes",
            "Yes.",
            "sí",
            "Sí",
            "si",
            "sim",
            "ok",
            "okay",
            "approve",
            "approved",
            "aprobar",
            "apruebo",
            "aprovar",
            "confirm",
            "confirmar",
            "confirmo",
            "do it",
            "hazlo",
            "faça",
            "allow",
            "permitir",
            "SERSHI, yes",
            "Sí, ciérralo",
        ] {
            let intent = KeywordIntentResolver.resolve(text);
            assert!(
                matches!(intent, Intent::NotUnderstood | Intent::Answer(_)),
                "{text}: {intent:?}"
            );
        }
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
