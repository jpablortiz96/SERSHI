//! Tier 1: deterministic command frames in English, Spanish and Portuguese.
//!
//! A frame is found only in command positions: a verb first (after fillers
//! such as "oye" or "please"), or a verb right after an expression of wanting
//! ("quiero abrir", "I need to open", "quero abrir"). A verb anywhere else —
//! "me gusta abrir Spotify", "estaba hablando de abrir Chrome" — is not a
//! command. A verb or wish directly negated ("no abras", "don't open", "não
//! quero abrir") produces [`Frame::Negated`], which never acts.
//!
//! The grammar reads the user's own tokens and hands the target on as the
//! user's text: it never invents or corrects a name. Resolving the target
//! against the trusted catalog is `super`'s job.

use crate::apps::normalize::normalize;

/// What the command asks to do with an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AppAction {
    Open,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// "<verb> <target>". `target` is the user's text (possibly empty).
    Command {
        action: AppAction,
        target: String,
    },
    /// A wish without a verb: "quiero Chrome", "I need Outlook". Only acts
    /// if the target is an exact trusted name.
    Wish {
        target: String,
    },
    /// "No abras Chrome", "don't open Outlook": never acts.
    Negated,
    None,
}

/// Imperative, infinitive and common spoken forms that open an application.
#[rustfmt::skip]
pub const OPEN_VERBS: &[&str] = &[
    /* en */ "open", "launch", "start", "run", "bring", "pull", "fire",
    /* es */ "abre", "abrir", "abra", "abres", "abras", "abreme", "abrame", "abrirme", "abrime",
             "inicia", "iniciar", "inicie", "ejecuta", "ejecutar", "ejecute", "pon", "ponme",
             "poner", "lanza", "lanzar", "arranca", "arrancar", "entra", "entrar", "prende",
    /* pt */ "abri", "executa", "execute", "executar", "rode", "rodar", "entre",
];
/// Verbs that mean "open" only after an explicit wish ("necesito revisar
/// Chrome", "I want to check Outlook").
#[rustfmt::skip]
const WEAK_OPEN_VERBS: &[&str] = &[
    "revisar", "usar", "use", "check", "checar", "utilizar",
];
#[rustfmt::skip]
pub const CLOSE_VERBS: &[&str] = &[
    /* en */ "close", "quit", "exit",
    /* es */ "cierra", "cerrar", "cierre", "cierras", "cierres", "cierrame", "termina", "terminar",
    /* pt */ "feche", "fechar", "fecha", "encerre", "encerrar",
];
/// Expressions of wanting or asking, after which a verb is a command.
#[rustfmt::skip]
const WISHES: &[&[&str]] = &[
    &["i", "would", "like", "to"], &["id", "like", "to"], &["i", "want", "to"], &["i", "need", "to"],
    &["i", "wanna"], &["i", "want"], &["i", "need"], &["want", "to"], &["need", "to"],
    &["can", "you"], &["could", "you"], &["would", "you"], &["please"], &["lets"],
    &["me", "gustaria"], &["quiero"], &["quisiera"], &["necesito"], &["puedes"], &["podrias"],
    &["podes"], &["vamos", "a"], &["por", "favor"],
    &["gostaria", "de"], &["quero"], &["queria"], &["preciso"], &["voce", "pode"], &["pode"],
    &["poderia"],
];
/// Words skipped between a wish and its verb ("quiero que abras", "can you
/// please open").
const WISH_GLUE: &[&str] = &["que", "tu", "you", "me", "to", "please", "de"];
/// Leading words that carry no meaning ("Oye, …", "OK SERSHI, …").
#[rustfmt::skip]
const FILLERS: &[&[&str]] = &[
    &["a", "ver"], &["oye"], &["oiga"], &["hey"], &["hola"], &["ola"], &["oi"], &["hi"], &["hello"],
    &["sershi"], &["ok"], &["okay"], &["bueno"], &["este"], &["eh"], &["em"], &["um"], &["uh"],
    &["ah"], &["mira"], &["olha"], &["ya"], &["entonces"], &["so"], &["well"], &["pues"], &["bom"],
];
const NEGATIONS: &[&str] = &[
    "no", "not", "dont", "never", "nunca", "jamas", "nao", "nem", "doesnt", "cant", "wont",
];
/// Words that stand alone before a comma without changing the command.
const INTERJECTIONS: &[&str] = &["no", "si", "yes", "sim", "nao", "vale", "dale"];
/// Short words that may sit between a negation and its verb ("no lo
/// abras", "não me abra").
const CLITICS: &[&str] = &[
    "lo", "la", "los", "las", "le", "les", "me", "te", "se", "nos", "o", "a", "you", "i", "to",
];
/// Words dropped at the start of a target ("abra o Chrome", "entra a Chrome").
#[rustfmt::skip]
const TARGET_LEADING: &[&str] = &[
    "the", "el", "la", "los", "las", "un", "una", "o", "a", "os", "as", "um", "uma", "al", "en",
    "no", "na", "em", "to", "into", "up", "de", "del", "do", "da", "mi", "my", "meu", "minha",
];
/// Words dropped at the end of a target ("open Spotify app please").
#[rustfmt::skip]
const TARGET_TRAILING: &[&str] = &[
    "app", "application", "aplicacion", "aplicativo", "programa", "please", "favor", "por",
    "porfa", "ahora", "now", "ya", "agora", "pls", "up",
];

/// A word as the user wrote it plus its comparison key.
#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub raw: &'a str,
    pub key: String,
    /// The raw word ended a clause (",", ".", ";", ":"), so a negation
    /// before it does not reach the next word ("No, abre Chrome").
    pub ends_clause: bool,
}

/// Whitespace-separated words. Punctuation-only words ("¿", "*") are kept
/// with an empty key, so a target keeps every character the user typed.
pub fn tokens(text: &str) -> Vec<Token<'_>> {
    text.split_whitespace()
        .map(|raw| Token {
            raw,
            ends_clause: raw.ends_with([',', '.', ';', ':', '!', '?']),
            key: normalize(raw),
        })
        .collect()
}

fn is_open(key: &str) -> bool {
    OPEN_VERBS.contains(&key)
}

fn is_close(key: &str) -> bool {
    CLOSE_VERBS.contains(&key)
}

fn verb(key: &str, weak_ok: bool) -> Option<AppAction> {
    if is_open(key) || (weak_ok && WEAK_OPEN_VERBS.contains(&key)) {
        Some(AppAction::Open)
    } else if is_close(key) {
        Some(AppAction::Close)
    } else {
        None
    }
}

/// Length of a phrase from `list` starting at `at`, if any (longest first).
fn phrase_at(tokens: &[Token<'_>], at: usize, list: &[&[&str]]) -> Option<usize> {
    list.iter()
        .filter(|p| {
            at + p.len() <= tokens.len()
                && p.iter()
                    .zip(&tokens[at..at + p.len()])
                    .all(|(w, t)| t.key == *w)
        })
        .map(|p| p.len())
        .max()
}

/// Index of the first token after leading fillers (including an
/// interjection that ends its own clause: "No, abre Chrome").
pub fn skip_fillers(tokens: &[Token<'_>]) -> usize {
    let mut at = 0;
    loop {
        if tokens.get(at).is_some_and(|t| t.key.is_empty()) {
            at += 1;
        } else if let Some(n) = phrase_at(tokens, at, FILLERS) {
            at += n;
        } else if at + 1 < tokens.len()
            && tokens[at].ends_clause
            && INTERJECTIONS.contains(&tokens[at].key.as_str())
        {
            at += 1;
        } else {
            return at;
        }
    }
}

fn negated_before(tokens: &[Token<'_>], at: usize) -> bool {
    let neg = |i: usize| NEGATIONS.contains(&tokens[i].key.as_str()) && !tokens[i].ends_clause;
    (at >= 1 && neg(at - 1))
        || (at >= 2
            && CLITICS.contains(&tokens[at - 1].key.as_str())
            && !tokens[at - 1].ends_clause
            && neg(at - 2))
}

pub fn parse(text: &str) -> Frame {
    let tokens = tokens(text);
    parse_tokens(&tokens)
}

pub fn parse_tokens(tokens: &[Token<'_>]) -> Frame {
    // A negated verb or wish anywhere: nothing may act.
    for i in 0..tokens.len() {
        let is_anchor =
            verb(&tokens[i].key, true).is_some() || phrase_at(tokens, i, WISHES).is_some();
        if is_anchor && negated_before(tokens, i) {
            return Frame::Negated;
        }
    }

    let start = skip_fillers(tokens);
    // Verb first ("Abre Chrome"), or a clitic then a verb ("¿Me abres Chrome?").
    let mut first = start;
    if first + 1 < tokens.len() && matches!(tokens[first].key.as_str(), "me" | "te" | "le") {
        first += 1;
    }
    if let Some(action) = tokens.get(first).and_then(|t| verb(&t.key, false)) {
        return Frame::Command {
            action,
            target: target(&tokens[first + 1..]),
        };
    }
    // A wish, then a verb or a bare target.
    let mut i = start;
    while i < tokens.len() {
        if let Some(n) = phrase_at(tokens, i, WISHES) {
            let mut j = i + n;
            while j < tokens.len() && WISH_GLUE.contains(&tokens[j].key.as_str()) {
                j += 1;
            }
            if let Some(action) = tokens.get(j).and_then(|t| verb(&t.key, true)) {
                return Frame::Command {
                    action,
                    target: target(&tokens[j + 1..]),
                };
            }
            // Only a wish at the start may take a bare target ("Quiero
            // Chrome"); mid-sentence wishes need a verb.
            if i == start && j < tokens.len() {
                let t = target(&tokens[j..]);
                if !t.is_empty() {
                    return Frame::Wish { target: t };
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    Frame::None
}

/// The target as the user said it, without leading articles, trailing
/// courtesy words or surrounding punctuation.
pub fn target(rest: &[Token<'_>]) -> String {
    let mut rest = rest;
    for _ in 0..2 {
        match rest.split_first() {
            Some((first, tail))
                if !tail.is_empty() && TARGET_LEADING.contains(&first.key.as_str()) =>
            {
                rest = tail;
            }
            _ => break,
        }
    }
    while let Some((last, head)) = rest.split_last() {
        if !head.is_empty() && TARGET_TRAILING.contains(&last.key.as_str()) {
            rest = head;
        } else {
            break;
        }
    }
    if rest.len() == 1 && TARGET_LEADING.contains(&rest[0].key.as_str()) {
        return String::new();
    }
    let joined = rest.iter().map(|t| t.raw).collect::<Vec<_>>().join(" ");
    joined
        .trim_matches(|c: char| c.is_whitespace() || ".,;:!?¿¡\"'“”«»()".contains(c))
        .to_owned()
}

/// The utterance without fillers or punctuation, as the user wrote it
/// ("Oye, Outlook." → "Outlook"), for bare application names.
pub fn bare(tokens: &[Token<'_>]) -> String {
    target(&tokens[skip_fillers(tokens)..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(text: &str) -> Option<(AppAction, String)> {
        match parse(text) {
            Frame::Command { action, target } => Some((action, target)),
            _ => None,
        }
    }

    #[test]
    fn natural_open_commands_in_three_languages() {
        for (text, app) in [
            ("Abre Chrome", "Chrome"),
            ("Abrir Chrome", "Chrome"),
            ("Ábreme Chrome", "Chrome"),
            ("Ponme Chrome", "Chrome"),
            ("Quiero abrir Chrome", "Chrome"),
            ("Necesito abrir Chrome", "Chrome"),
            ("Oye, quiero abrir Google Chrome", "Google Chrome"),
            (
                "Oye, no sé si me escuchas que quiero abrir Google Chrome",
                "Google Chrome",
            ),
            ("Necesito entrar a Chrome", "Chrome"),
            ("Necesito revisar Chrome", "Chrome"),
            ("¿Me abres Chrome?", "Chrome"),
            ("Open Chrome", "Chrome"),
            ("Launch Chrome", "Chrome"),
            ("Bring up Chrome", "Chrome"),
            ("Start Excel", "Excel"),
            ("I want to open Outlook", "Outlook"),
            ("Can you please open Word?", "Word"),
            ("Abra o Chrome", "Chrome"),
            ("Abre o Chrome", "Chrome"),
            ("Quero abrir o Chrome", "Chrome"),
            ("Abra o Visual Studio Code, por favor", "Visual Studio Code"),
        ] {
            assert_eq!(
                command(text),
                Some((AppAction::Open, app.to_owned())),
                "{text}"
            );
        }
    }

    #[test]
    fn close_commands() {
        for text in [
            "Cierra Outlook",
            "Close Outlook",
            "Feche o Outlook",
            "Quiero cerrar Outlook",
        ] {
            assert_eq!(
                command(text),
                Some((AppAction::Close, "Outlook".to_owned())),
                "{text}"
            );
        }
    }

    #[test]
    fn wishes_without_a_verb() {
        for (text, app) in [
            ("Quiero Chrome", "Chrome"),
            ("Necesito Chrome", "Chrome"),
            ("I need Outlook", "Outlook"),
            ("Quero o Excel", "Excel"),
        ] {
            assert_eq!(
                parse(text),
                Frame::Wish {
                    target: app.to_owned()
                },
                "{text}"
            );
        }
    }

    #[test]
    fn negations_never_become_commands() {
        for text in [
            "No abras Chrome",
            "No quiero abrir Outlook",
            "No lo abras",
            "Don't open Chrome",
            "Please do not open Outlook",
            "Never open Word",
            "Não abra o Chrome",
            "Não quero abrir o Outlook",
        ] {
            assert_eq!(parse(text), Frame::Negated, "{text}");
        }
        // A "no" that ends its own clause does not negate the command.
        assert_eq!(
            command("No, abre Chrome"),
            Some((AppAction::Open, "Chrome".to_owned()))
        );
    }

    #[test]
    fn mentions_are_not_commands() {
        for text in [
            "Outlook es muy lento.",
            "Chrome es mi navegador favorito.",
            "¿Qué es Microsoft Word?",
            "Estaba hablando de PowerShell.",
            "La aplicación Outlook se cerró.",
            "Me gusta abrir Spotify",
            "Estaba hablando de abrir Chrome",
            "the app to open is Spotify",
            "opening Spotify later",
        ] {
            assert!(
                matches!(parse(text), Frame::None),
                "{text}: {:?}",
                parse(text)
            );
        }
    }

    #[test]
    fn targets_keep_the_users_words() {
        assert_eq!(
            command("open cmd /c del *"),
            Some((AppAction::Open, "cmd /c del *".to_owned()))
        );
        assert_eq!(command("Abre"), Some((AppAction::Open, String::new())));
        assert_eq!(command("abra o"), Some((AppAction::Open, String::new())));
        assert_eq!(bare(&tokens("Oye, Outlook.")), "Outlook");
    }
}
