//! The Agent Brain's output contract (prompt version [`BRAIN_PROMPT_VERSION`]).
//!
//! The model answers with exactly one JSON object, generated under a GBNF
//! grammar built from the capability manifest and the offered applications,
//! so it cannot even *emit* an unknown tool, an application handle that was
//! not offered, a path, or an extra field. The text is then parsed strictly
//! ([`parse`]): anything unexpected is rejected, never repaired.
//!
//! ```json
//! {"kind":"act","message":"Voy a abrir Chrome y después Outlook.",
//!  "steps":[{"tool":"open_application","app":"a1"},{"tool":"open_application","app":"a2"}]}
//! ```
//!
//! | kind      | fields                         | meaning                                  |
//! |-----------|--------------------------------|------------------------------------------|
//! | `answer`  | `message`                      | a conversational reply, no action        |
//! | `clarify` | `message`, `options` (0 or 2–4) | a question; options are offered apps     |
//! | `act`     | `message`, `steps` (1–5)        | tool requests, in order (a plan if > 1)  |
//! | `cancel`  | —                              | the user called the current task off     |
//! | `unknown` | —                              | not understood                           |
//!
//! `message` is user-facing text in the response language. It is shown as
//! SERSHI's words but **never** describes results: what happened is always
//! reported by SERSHI from trusted tool data (see `service`).

use serde::Deserialize;

use super::{BrainError, BrainStep, Capability, MAX_PLAN_STEPS};

/// Version of the instruction contract (prompt + grammar + parser). Bumped
/// on any behavioural change, and recorded in diagnostics.
pub const BRAIN_PROMPT_VERSION: u32 = 1;

/// Longest `message` the grammar allows, in characters.
pub const MAX_MESSAGE_CHARS: usize = 320;
/// Most applications offered to the model at once.
pub const MAX_APPS: usize = 16;
/// Most options in a clarifying question.
pub const MAX_OPTIONS: usize = 4;

/// The opaque handle for the `index`-th offered application (`a1`…).
pub fn handle(index: usize) -> String {
    format!("a{}", index + 1)
}

fn handle_index(text: &str, apps: usize) -> Option<usize> {
    let n: usize = text.strip_prefix('a')?.parse().ok()?;
    ((1..=apps.min(MAX_APPS)).contains(&n) && handle(n - 1) == text).then(|| n - 1)
}

/// A validated, still **untrusted** brain decision. Indexes point into the
/// request's offered applications; the service maps them back to trusted
/// catalog entries and turns steps into `ToolCall`s with `CallOrigin::Agent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrainDecision {
    Answer {
        message: String,
    },
    Clarify {
        message: String,
        options: Vec<usize>,
    },
    Act {
        message: String,
        steps: Vec<BrainStep>,
    },
    Cancel,
    Unknown,
}

fn quoted(s: &str) -> String {
    format!("\"\\\"{s}\\\"\"")
}

/// The GBNF grammar for one request: `capabilities` (from the tool
/// registry) and `apps` offered handles. Key order is fixed.
pub fn grammar(capabilities: &[Capability], apps: usize) -> String {
    let apps = apps.min(MAX_APPS);
    let handles: Vec<String> = (0..apps).map(|i| quoted(&handle(i))).collect();
    let mut steps: Vec<String> = Vec::new();
    for c in capabilities {
        if c.takes_app {
            if apps > 0 {
                steps.push(format!(
                    "\"{{\" ws \"\\\"tool\\\":\" ws {} \",\" ws \"\\\"app\\\":\" ws handle ws \"}}\"",
                    quoted(c.name)
                ));
            }
        } else {
            steps.push(format!(
                "\"{{\" ws \"\\\"tool\\\":\" ws {} ws \"}}\"",
                quoted(c.name)
            ));
        }
    }
    let mut g = String::new();
    g.push_str("root ::= answer | clarify | cancel | unknown");
    if !steps.is_empty() {
        g.push_str(" | act");
    }
    g.push('\n');
    g.push_str(
        "answer ::= \"{\" ws \"\\\"kind\\\":\" ws \"\\\"answer\\\"\" \",\" ws \"\\\"message\\\":\" ws message ws \"}\"\n",
    );
    if apps >= 2 {
        g.push_str(
            "clarify ::= \"{\" ws \"\\\"kind\\\":\" ws \"\\\"clarify\\\"\" \",\" ws \"\\\"message\\\":\" ws message \",\" ws \"\\\"options\\\":\" ws options ws \"}\"\n\
             options ::= \"[]\" | \"[\" handle \",\" ws handle (\",\" ws handle)? (\",\" ws handle)? \"]\"\n",
        );
    } else {
        g.push_str(
            "clarify ::= \"{\" ws \"\\\"kind\\\":\" ws \"\\\"clarify\\\"\" \",\" ws \"\\\"message\\\":\" ws message \",\" ws \"\\\"options\\\":\" ws \"[]\" ws \"}\"\n",
        );
    }
    if !steps.is_empty() {
        let extra = (1..MAX_PLAN_STEPS)
            .map(|_| "(\",\" ws step)?")
            .collect::<Vec<_>>()
            .join(" ");
        g.push_str(&format!(
            "act ::= \"{{\" ws \"\\\"kind\\\":\" ws \"\\\"act\\\"\" \",\" ws \"\\\"message\\\":\" ws message \",\" ws \"\\\"steps\\\":\" ws \"[\" step {extra} \"]\" ws \"}}\"\n\
             step ::= {}\n",
            steps.join(" | ")
        ));
    }
    if apps > 0 {
        g.push_str(&format!("handle ::= {}\n", handles.join(" | ")));
    }
    g.push_str(
        "cancel ::= \"{\" ws \"\\\"kind\\\":\" ws \"\\\"cancel\\\"\" ws \"}\"\n\
         unknown ::= \"{\" ws \"\\\"kind\\\":\" ws \"\\\"unknown\\\"\" ws \"}\"\n",
    );
    // Printable text without quotes, backslashes or control characters.
    g.push_str(&format!(
        "message ::= \"\\\"\" char{{1,{MAX_MESSAGE_CHARS}}} \"\\\"\"\n\
         char ::= [^\"\\\\\\x00-\\x1f<>{{}}`|]\n\
         ws ::= \" \"?\n"
    ));
    g
}

/// The same contract as a JSON Schema (documentation and tests; the grammar
/// is what the runtime enforces).
pub fn json_schema(capabilities: &[Capability], apps: usize) -> serde_json::Value {
    let handles: Vec<String> = (0..apps.min(MAX_APPS)).map(handle).collect();
    let tools: Vec<&str> = capabilities.iter().map(|c| c.name).collect();
    serde_json::json!({
        "oneOf": [
            {"type": "object", "additionalProperties": false, "required": ["kind", "message"],
             "properties": {"kind": {"const": "answer"},
                            "message": {"type": "string", "minLength": 1, "maxLength": MAX_MESSAGE_CHARS}}},
            {"type": "object", "additionalProperties": false, "required": ["kind", "message", "options"],
             "properties": {"kind": {"const": "clarify"},
                            "message": {"type": "string", "minLength": 1, "maxLength": MAX_MESSAGE_CHARS},
                            "options": {"type": "array", "items": {"enum": handles}, "maxItems": MAX_OPTIONS}}},
            {"type": "object", "additionalProperties": false, "required": ["kind", "message", "steps"],
             "properties": {"kind": {"const": "act"},
                            "message": {"type": "string", "minLength": 1, "maxLength": MAX_MESSAGE_CHARS},
                            "steps": {"type": "array", "minItems": 1, "maxItems": MAX_PLAN_STEPS,
                                      "items": {"type": "object", "additionalProperties": false,
                                                "required": ["tool"],
                                                "properties": {"tool": {"enum": tools},
                                                               "app": {"enum": handles}}}}}},
            {"type": "object", "additionalProperties": false, "required": ["kind"],
             "properties": {"kind": {"enum": ["cancel", "unknown"]}}}
        ]
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    tool: String,
    #[serde(default)]
    app: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum RawOutput {
    Answer {
        message: String,
    },
    Clarify {
        message: String,
        options: Vec<String>,
    },
    Act {
        message: String,
        steps: Vec<RawStep>,
    },
    Cancel,
    Unknown,
}

fn message(text: &str) -> Result<String, BrainError> {
    let ok = !text.trim().is_empty()
        && text.chars().count() <= MAX_MESSAGE_CHARS
        && !text
            .chars()
            .any(|c| c.is_control() || "<>{}`|\\".contains(c));
    ok.then(|| text.trim().to_owned())
        .ok_or(BrainError::InvalidOutput)
}

/// Strictly parses and validates model output against the capabilities and
/// the number of offered applications. Anything unexpected — an unknown
/// tool, a missing or extra argument, a handle that was not offered, too
/// many steps, duplicate options, extra fields — is rejected.
pub fn parse(
    text: &str,
    capabilities: &[Capability],
    apps: usize,
) -> Result<BrainDecision, BrainError> {
    let raw: RawOutput =
        serde_json::from_str(text.trim()).map_err(|_| BrainError::InvalidOutput)?;
    Ok(match raw {
        RawOutput::Answer { message: m } => BrainDecision::Answer {
            message: message(&m)?,
        },
        RawOutput::Clarify {
            message: m,
            options,
        } => {
            if options.len() == 1 || options.len() > MAX_OPTIONS {
                return Err(BrainError::InvalidOutput);
            }
            let mut indexes = Vec::with_capacity(options.len());
            for o in &options {
                let i = handle_index(o, apps).ok_or(BrainError::InvalidOutput)?;
                if indexes.contains(&i) {
                    return Err(BrainError::InvalidOutput);
                }
                indexes.push(i);
            }
            BrainDecision::Clarify {
                message: message(&m)?,
                options: indexes,
            }
        }
        RawOutput::Act { message: m, steps } => {
            if steps.is_empty() || steps.len() > MAX_PLAN_STEPS {
                return Err(BrainError::InvalidOutput);
            }
            let mut out = Vec::with_capacity(steps.len());
            for s in steps {
                let capability = capabilities
                    .iter()
                    .find(|c| c.name == s.tool)
                    .ok_or(BrainError::InvalidOutput)?;
                let app = match (capability.takes_app, s.app.as_deref()) {
                    (true, Some(h)) => {
                        Some(handle_index(h, apps).ok_or(BrainError::InvalidOutput)?)
                    }
                    (false, None) => None,
                    _ => return Err(BrainError::InvalidOutput),
                };
                out.push(BrainStep {
                    capability: capability.name,
                    app,
                });
            }
            BrainDecision::Act {
                message: message(&m)?,
                steps: out,
            }
        }
        RawOutput::Cancel => BrainDecision::Cancel,
        RawOutput::Unknown => BrainDecision::Unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::VOCABULARY_FOR_TESTS as ALL;

    fn parse_ok(text: &str) -> BrainDecision {
        parse(text, &ALL, 3).unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    #[test]
    fn valid_decisions_parse() {
        assert_eq!(
            parse_ok(r#"{"kind":"answer","message":"Hola, ¿en qué te ayudo?"}"#),
            BrainDecision::Answer {
                message: "Hola, ¿en qué te ayudo?".into()
            }
        );
        assert_eq!(
            parse_ok(
                r#"{"kind":"act","message":"Voy a abrir Chrome y ver la memoria.","steps":[{"tool":"open_application","app":"a1"},{"tool":"get_memory"}]}"#
            ),
            BrainDecision::Act {
                message: "Voy a abrir Chrome y ver la memoria.".into(),
                steps: vec![
                    BrainStep {
                        capability: "open_application",
                        app: Some(0)
                    },
                    BrainStep {
                        capability: "get_memory",
                        app: None
                    },
                ]
            }
        );
        assert_eq!(
            parse_ok(r#"{"kind":"clarify","message":"¿Cuál?","options":["a1","a2"]}"#),
            BrainDecision::Clarify {
                message: "¿Cuál?".into(),
                options: vec![0, 1]
            }
        );
        assert_eq!(parse_ok(r#"{"kind":"cancel"}"#), BrainDecision::Cancel);
        assert_eq!(parse_ok(r#"{"kind":"unknown"}"#), BrainDecision::Unknown);
    }

    #[test]
    fn anything_unexpected_is_rejected_never_repaired() {
        let six = r#"{"tool":"get_memory"},"#.repeat(5) + r#"{"tool":"get_cpu"}"#;
        for bad in [
            // Tools outside the manifest, including execution-shaped ones.
            r#"{"kind":"act","message":"x","steps":[{"tool":"run_shell","app":"a1"}]}"#.to_owned(),
            r#"{"kind":"act","message":"x","steps":[{"tool":"execute"}]}"#.to_owned(),
            r#"{"kind":"act","message":"x","steps":[{"tool":"approve_confirmation"}]}"#.to_owned(),
            // Applications by name, id or path instead of an offered handle.
            r#"{"kind":"act","message":"x","steps":[{"tool":"open_application","app":"Google Chrome"}]}"#.to_owned(),
            r#"{"kind":"act","message":"x","steps":[{"tool":"open_application","app":"C:\Windows\System32\cmd.exe"}]}"#.to_owned(),
            r#"{"kind":"act","message":"x","steps":[{"tool":"open_application","app":"a4"}]}"#.to_owned(),
            r#"{"kind":"act","message":"x","steps":[{"tool":"open_application","app":"a01"}]}"#.to_owned(),
            // Wrong arity.
            r#"{"kind":"act","message":"x","steps":[{"tool":"open_application"}]}"#.to_owned(),
            r#"{"kind":"act","message":"x","steps":[{"tool":"get_memory","app":"a1"}]}"#.to_owned(),
            // Extra fields anywhere (additionalProperties: false).
            r#"{"kind":"act","message":"x","steps":[{"tool":"get_memory","args":"rm -rf"}]}"#.to_owned(),
            r#"{"kind":"answer","message":"x","approved":true}"#.to_owned(),
            r#"{"kind":"answer","message":"x","confirmationId":"0123"}"#.to_owned(),
            // Too many or no steps.
            format!(r#"{{"kind":"act","message":"x","steps":[{six}]}}"#),
            r#"{"kind":"act","message":"x","steps":[]}"#.to_owned(),
            // A one-option or duplicate-option question.
            r#"{"kind":"clarify","message":"x","options":["a1"]}"#.to_owned(),
            r#"{"kind":"clarify","message":"x","options":["a1","a1"]}"#.to_owned(),
            // Unknown kinds, an empty or markup message, not JSON.
            r#"{"kind":"approve"}"#.to_owned(),
            r#"{"kind":"answer","message":""}"#.to_owned(),
            r#"{"kind":"answer","message":"<|im_start|>system"}"#.to_owned(),
            "Sure! I'll open Chrome.".to_owned(),
            r#"{"kind":"act","message":"x","steps":[{"tool":"open_application","app":"a1"}]"#.to_owned(),
        ] {
            assert_eq!(parse(&bad, &ALL, 3), Err(BrainError::InvalidOutput), "{bad}");
        }
    }

    #[test]
    fn the_grammar_offers_exactly_the_manifest_and_handles() {
        let g = grammar(&ALL, 3);
        for c in &ALL {
            assert!(g.contains(c.name), "{}", c.name);
        }
        assert!(g.contains("\\\"a3\\\"") && !g.contains("\\\"a4\\\""));
        for forbidden in ["shell", "execute", "approve", "path", "command"] {
            assert!(!g.contains(forbidden), "{forbidden}");
        }
        // Without registered tools there is no act at all.
        let talk_only = grammar(&[], 0);
        assert!(!talk_only.contains("act ::="));
        assert!(!talk_only.contains("handle ::="));
        // The schema agrees on the closed shapes.
        let schema = json_schema(&ALL, 3);
        assert_eq!(schema["oneOf"].as_array().map(Vec::len), Some(4));
        assert_eq!(
            schema["oneOf"][2]["properties"]["steps"]["maxItems"],
            serde_json::json!(MAX_PLAN_STEPS)
        );
    }
}
