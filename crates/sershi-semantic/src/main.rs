//! `sershi-semantic`: SERSHI's local semantic-model runtime.
//!
//! SERSHI starts this process itself (never a shell), keeps it private to
//! its own pipes, and ends it when the model has been idle for a while or
//! SERSHI exits (a Windows job object kills it with SERSHI). It exists as a
//! separate process so that:
//!
//! - llama.cpp's copy of ggml never meets whisper.cpp's copy inside SERSHI;
//! - unloading the model returns every byte of RAM and video memory;
//! - a crash or out-of-memory in inference cannot take SERSHI down.
//!
//! It can do exactly two things: load one GGUF model file that SERSHI has
//! already verified, and generate text constrained by a grammar SERSHI
//! supplies. It has no tools, no network, no file writes and no knowledge
//! of SERSHI. Its output is untrusted data that SERSHI parses strictly.
//!
//! Protocol: one JSON object per line on stdin, one JSON reply per line on
//! stdout (see [`Request`]). End of input ends the process.

use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod engine;

/// Longest request line accepted (bytes).
const MAX_LINE: usize = 64 * 1024;
const MAX_PROMPT: usize = 32 * 1024;
const MAX_GRAMMAR: usize = 8 * 1024;
const MAX_TOKENS: u32 = 256;
const MAX_CONTEXT: u32 = 8192;
const MAX_THREADS: u32 = 16;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    /// Load the model (once per process).
    Load {
        model: PathBuf,
        gpu: bool,
        threads: u32,
        context: u32,
    },
    /// Generate under a GBNF grammar.
    Generate {
        prompt: String,
        grammar: String,
        max_tokens: u32,
    },
    Shutdown,
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// "vulkan" or "cpu" (load).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    /// Prompt tokens reused from the previous request (shared prefix).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u32>,
}

impl Response {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            error: Some(message.into()),
            ..Self::default()
        }
    }
}

/// Validates a request's bounds before anything touches llama.cpp.
pub fn validate(request: &Request) -> Result<(), String> {
    match request {
        Request::Load {
            model,
            threads,
            context,
            ..
        } => {
            if !model.is_absolute()
                || model
                    .extension()
                    .is_none_or(|e| !e.eq_ignore_ascii_case("gguf"))
            {
                return Err("model must be an absolute .gguf path".to_owned());
            }
            if !(1..=MAX_THREADS).contains(threads) || !(256..=MAX_CONTEXT).contains(context) {
                return Err("invalid threads or context".to_owned());
            }
            if !model.is_file() {
                return Err("model not found".to_owned());
            }
            Ok(())
        }
        Request::Generate {
            prompt,
            grammar,
            max_tokens,
        } => {
            if prompt.is_empty() || prompt.len() > MAX_PROMPT || grammar.len() > MAX_GRAMMAR {
                return Err("invalid prompt or grammar size".to_owned());
            }
            if !(1..=MAX_TOKENS).contains(max_tokens) {
                return Err("invalid token budget".to_owned());
            }
            Ok(())
        }
        Request::Shutdown => Ok(()),
    }
}

fn main() {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    #[cfg(windows)]
    let mut engine = engine::Engine::default();
    let mut line = String::new();
    loop {
        line.clear();
        match input
            .by_ref()
            .take(MAX_LINE as u64 + 1)
            .read_line(&mut line)
        {
            Ok(0) | Err(_) => return, // SERSHI went away.
            Ok(n) if n > MAX_LINE => {
                reply(&stdout, &Response::error("request too long"));
                return;
            }
            Ok(_) => {}
        }
        let response = match serde_json::from_str::<Request>(line.trim()) {
            Err(_) => Response::error("malformed request"),
            Ok(Request::Shutdown) => return,
            Ok(request) => match validate(&request) {
                Err(e) => Response::error(e),
                #[cfg(windows)]
                Ok(()) => engine.handle(request),
                #[cfg(not(windows))]
                Ok(()) => Response::error("unsupported platform"),
            },
        };
        if !reply(&stdout, &response) {
            return;
        }
    }
}

fn reply(stdout: &io::Stdout, response: &Response) -> bool {
    let Ok(json) = serde_json::to_string(response) else {
        return false;
    };
    let mut out = stdout.lock();
    writeln!(out, "{json}").and_then(|()| out.flush()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_strict_and_bounded() {
        let load: Request = serde_json::from_str(
            r#"{"op":"load","model":"C:\\m\\x.gguf","gpu":true,"threads":4,"context":2048}"#,
        )
        .unwrap();
        assert!(matches!(load, Request::Load { gpu: true, .. }));
        for bad in [
            r#"{"op":"load","model":"x.gguf","gpu":true,"threads":4,"context":2048,"extra":1}"#,
            r#"{"op":"exec","command":"cmd"}"#,
            r#"{"op":"generate","prompt":"p"}"#,
        ] {
            assert!(serde_json::from_str::<Request>(bad).is_err(), "{bad}");
        }
        let relative = Request::Load {
            model: "model.gguf".into(),
            gpu: false,
            threads: 4,
            context: 2048,
        };
        assert!(validate(&relative).is_err());
        let not_gguf = Request::Load {
            model: std::env::temp_dir().join("model.exe"),
            gpu: false,
            threads: 4,
            context: 2048,
        };
        assert!(validate(&not_gguf).is_err());
        let huge = Request::Generate {
            prompt: "x".repeat(MAX_PROMPT + 1),
            grammar: String::new(),
            max_tokens: 16,
        };
        assert!(validate(&huge).is_err());
        let budget = Request::Generate {
            prompt: "x".into(),
            grammar: String::new(),
            max_tokens: MAX_TOKENS + 1,
        };
        assert!(validate(&budget).is_err());
    }

    #[test]
    fn responses_omit_empty_fields() {
        let json = serde_json::to_string(&Response::error("x")).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"x"}"#);
    }
}
