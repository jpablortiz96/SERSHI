//! The local semantic model's runtime: the `sershi-semantic` process.
//!
//! [`EngineGenerator`] implements `sershi_core`'s `TextGenerator` by
//! driving `sershi-semantic.exe` (installed next to SERSHI) over its private
//! pipes. The process is started directly — never through a shell — with no
//! console window, below-normal CPU priority (so inference never makes the
//! user's own applications sluggish) and, on Windows, inside a job object
//! that ends it with SERSHI.
//!
//! Lifecycle: lazy load (on first use, or when the microphone opens), warm
//! while in use, and the whole process — model, RAM and video memory — is
//! released after [`IDLE_RELEASE`] without requests. A hung or crashed
//! engine is killed and restarted on the next request; every failure is an
//! error the caller falls back from (deterministic understanding keeps
//! working).

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sershi_core::understanding::semantic::{Generation, RouterError, TextGenerator};

/// A warm engine is released after this long without requests. Loading
/// again takes ~1–3 s (measured, docs/SEMANTIC.md), so holding gigabytes of
/// memory for longer is not worth it.
pub const IDLE_RELEASE: Duration = Duration::from_secs(5 * 60);
/// Longest line read from the engine.
const MAX_LINE: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// `sershi-semantic.exe` next to SERSHI's own executable.
    pub program: PathBuf,
    /// A model file SERSHI has verified (size and SHA-256).
    pub model: PathBuf,
    pub gpu: bool,
    pub threads: u32,
    pub context: u32,
    /// Starting the process and loading the model.
    pub load_timeout: Duration,
    /// One generation.
    pub request_timeout: Duration,
    /// The engine's expected SHA-256 (installer builds): an engine that does
    /// not match is never started.
    pub program_sha256: Option<&'static str>,
}

impl EngineConfig {
    pub fn new(program: PathBuf, model: PathBuf, gpu: bool) -> Self {
        let logical = thread::available_parallelism().map_or(4, |n| n.get());
        Self {
            program,
            model,
            gpu,
            // Like speech recognition: a few threads on the GPU, half the
            // cores (at most six) on the CPU.
            threads: u32::try_from(if gpu { 4 } else { (logical / 2).clamp(2, 6) }).unwrap_or(4),
            context: 2048,
            load_timeout: Duration::from_secs(60),
            request_timeout: Duration::from_secs(if gpu { 5 } else { 12 }),
            program_sha256: None,
        }
    }
}

/// What the running engine reported (developer diagnostics).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub running: bool,
    pub backend: Option<String>,
    pub device: Option<String>,
    pub load_ms: Option<u32>,
    pub last_ms: Option<u32>,
    pub last_prompt_tokens: Option<u32>,
    pub last_cached_tokens: Option<u32>,
    pub last_generated_tokens: Option<u32>,
}

#[derive(Serialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum Request<'a> {
    Load {
        model: &'a std::path::Path,
        gpu: bool,
        threads: u32,
        context: u32,
    },
    Generate {
        prompt: &'a str,
        grammar: &'a str,
        max_tokens: u32,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    ok: bool,
    error: Option<String>,
    backend: Option<String>,
    device: Option<String>,
    text: Option<String>,
    prompt_tokens: Option<u32>,
    cached_tokens: Option<u32>,
    generated_tokens: Option<u32>,
    ms: Option<u32>,
}

struct Process {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Process {
    fn request(
        &mut self,
        request: &Request<'_>,
        timeout: Duration,
    ) -> Result<Response, RouterError> {
        let line = serde_json::to_string(request).map_err(|_| RouterError::Failed)?;
        writeln!(self.stdin, "{line}")
            .and_then(|()| self.stdin.flush())
            .map_err(|_| RouterError::Unavailable)?;
        let reply = match self.lines.recv_timeout(timeout) {
            Ok(reply) => reply,
            Err(RecvTimeoutError::Timeout) => return Err(RouterError::Timeout),
            Err(RecvTimeoutError::Disconnected) => return Err(RouterError::Unavailable),
        };
        serde_json::from_str(&reply).map_err(|_| RouterError::Failed)
    }
}

#[derive(Default)]
struct State {
    process: Option<Process>,
    last_used: Option<Instant>,
    status: EngineStatus,
}

/// `TextGenerator` over the `sershi-semantic` process.
#[derive(Clone)]
pub struct EngineGenerator {
    config: EngineConfig,
    state: Arc<Mutex<State>>,
    /// The engine executable matched `config.program_sha256`.
    engine_ok: Arc<std::sync::OnceLock<bool>>,
}

impl std::fmt::Debug for EngineGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineGenerator")
            .field("gpu", &self.config.gpu)
            .finish_non_exhaustive()
    }
}

impl EngineGenerator {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(State::default())),
            engine_ok: Arc::new(std::sync::OnceLock::new()),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, RouterError> {
        self.state.lock().map_err(|_| RouterError::Unavailable)
    }

    pub fn status(&self) -> EngineStatus {
        self.lock().map(|s| s.status.clone()).unwrap_or_default()
    }

    /// Ends the engine after [`IDLE_RELEASE`] (or `idle`) without requests,
    /// returning its memory. Returns whether it was released.
    pub fn release_if_idle(&self, idle: Duration) -> bool {
        let Ok(mut state) = self.state.try_lock() else {
            return false; // Busy: not idle.
        };
        let due = state.process.is_some() && state.last_used.is_none_or(|t| t.elapsed() >= idle);
        if due {
            state.process = None;
            state.status.running = false;
        }
        due
    }

    /// Ends the engine now (settings changed, SERSHI quitting).
    pub fn release(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.process = None;
            state.status.running = false;
        }
    }

    fn ensure(&self, state: &mut State) -> Result<(), RouterError> {
        if state.process.is_some() {
            return Ok(());
        }
        let mut process = self.spawn()?;
        let reply = process.request(
            &Request::Load {
                model: &self.config.model,
                gpu: self.config.gpu,
                threads: self.config.threads,
                context: self.config.context,
            },
            self.config.load_timeout,
        )?;
        let (process, reply) = if !reply.ok && self.config.gpu {
            // The GPU could not take the model (e.g. not enough video memory
            // next to speech recognition): adapt and use the CPU.
            eprintln!(
                "SERSHI semantic: GPU load failed ({}); using the CPU",
                reply.error.as_deref().unwrap_or("unknown")
            );
            drop(process);
            let mut process = self.spawn()?;
            let reply = process.request(
                &Request::Load {
                    model: &self.config.model,
                    gpu: false,
                    threads: EngineConfig::new(PathBuf::new(), PathBuf::new(), false).threads,
                    context: self.config.context,
                },
                self.config.load_timeout,
            )?;
            (process, reply)
        } else {
            (process, reply)
        };
        if !reply.ok {
            eprintln!(
                "SERSHI semantic: load failed ({})",
                reply.error.as_deref().unwrap_or("unknown")
            );
            return Err(RouterError::Unavailable);
        }
        state.status = EngineStatus {
            running: true,
            backend: reply.backend,
            device: reply.device,
            load_ms: reply.ms,
            ..EngineStatus::default()
        };
        state.process = Some(process);
        Ok(())
    }

    fn spawn(&self) -> Result<Process, RouterError> {
        if !self.config.program.is_file() || !self.config.model.is_file() {
            return Err(RouterError::NotInstalled);
        }
        if let Some(expected) = self.config.program_sha256
            && !self.engine_verified(expected)
        {
            eprintln!(
                "SERSHI semantic: the inference engine does not match this build; not started"
            );
            return Err(RouterError::Unavailable);
        }
        let mut command = Command::new(&self.config.program);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
            command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
        }
        let mut child = command.spawn().map_err(|_| RouterError::Unavailable)?;
        #[cfg(windows)]
        crate::windows::job::contain(&child);
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err(RouterError::Unavailable);
        };
        let (tx, lines) = channel();
        thread::Builder::new()
            .name("sershi-semantic-reader".to_owned())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.by_ref().take(MAX_LINE as u64).read_line(&mut line) {
                        Ok(0) | Err(_) => return,
                        Ok(_) => {
                            if tx.send(line.trim_end().to_owned()).is_err() {
                                return;
                            }
                        }
                    }
                }
            })
            .map_err(|_| RouterError::Unavailable)?;
        Ok(Process {
            child,
            stdin,
            lines,
        })
    }
}

impl EngineGenerator {
    /// Hashes the engine executable once per generator (it is code, not
    /// data: an installer build only runs the engine it shipped with).
    fn engine_verified(&self, expected: &str) -> bool {
        use sha2::{Digest, Sha256};
        if self.engine_ok.get() == Some(&true) {
            return true;
        }
        let ok = std::fs::read(&self.config.program).is_ok_and(|bytes| {
            let digest = Sha256::digest(&bytes);
            let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
            hex == expected
        });
        if ok {
            let _ = self.engine_ok.set(true);
        }
        ok
    }
}

impl TextGenerator for EngineGenerator {
    fn generate(&self, generation: &Generation) -> Result<String, RouterError> {
        let mut state = self.lock()?;
        self.ensure(&mut state)?;
        state.last_used = Some(Instant::now());
        let result = match state.process.as_mut() {
            Some(process) => process.request(
                &Request::Generate {
                    prompt: &generation.prompt,
                    grammar: &generation.grammar,
                    max_tokens: generation.max_tokens,
                },
                self.config.request_timeout,
            ),
            None => Err(RouterError::Unavailable),
        };
        match result {
            Ok(Response {
                ok: true,
                text: Some(text),
                prompt_tokens,
                cached_tokens,
                generated_tokens,
                ms,
                ..
            }) => {
                state.status.last_ms = ms;
                state.status.last_prompt_tokens = prompt_tokens;
                state.status.last_cached_tokens = cached_tokens;
                state.status.last_generated_tokens = generated_tokens;
                state.last_used = Some(Instant::now());
                Ok(text)
            }
            Ok(_) => Err(RouterError::Failed),
            Err(e) => {
                // Hung or dead: kill it; the next request starts afresh.
                state.process = None;
                state.status.running = false;
                Err(e)
            }
        }
    }

    fn prepare(&self) {
        let this = self.clone();
        let _ = thread::Builder::new()
            .name("sershi-semantic-load".to_owned())
            .spawn(move || {
                if let Ok(mut state) = this.state.try_lock() {
                    let _ = this.ensure(&mut state);
                    state.last_used = Some(Instant::now());
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_engine_or_model_is_not_installed() {
        let generator = EngineGenerator::new(EngineConfig::new(
            PathBuf::from("Z:\\nowhere\\sershi-semantic.exe"),
            PathBuf::from("Z:\\nowhere\\model.gguf"),
            false,
        ));
        let generation = Generation {
            prompt: "p".into(),
            grammar: "root ::= \"x\"".into(),
            max_tokens: 4,
        };
        assert_eq!(
            generator.generate(&generation),
            Err(RouterError::NotInstalled)
        );
        assert!(!generator.status().running);
        assert!(!generator.release_if_idle(Duration::ZERO));
    }

    #[test]
    fn an_engine_that_is_not_the_shipped_one_is_never_started() {
        let dir = std::env::temp_dir().join(format!("sershi-engine-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join("sershi-semantic.exe");
        let model = dir.join("model.gguf");
        std::fs::write(&program, b"not the engine").unwrap();
        std::fs::write(&model, b"gguf").unwrap();
        let mut config = EngineConfig::new(program, model, false);
        config.program_sha256 =
            Some("0000000000000000000000000000000000000000000000000000000000000000");
        let generator = EngineGenerator::new(config);
        let generation = Generation {
            prompt: "p".into(),
            grammar: "root ::= \"x\"".into(),
            max_tokens: 4,
        };
        assert_eq!(
            generator.generate(&generation),
            Err(RouterError::Unavailable)
        );
        assert!(!generator.status().running);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn requests_serialize_as_the_engine_expects() {
        let json = serde_json::to_string(&Request::Generate {
            prompt: "p",
            grammar: "g",
            max_tokens: 64,
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"op":"generate","prompt":"p","grammar":"g","maxTokens":64}"#
        );
    }

    #[test]
    fn cpu_engines_use_bounded_threads() {
        let config = EngineConfig::new(PathBuf::new(), PathBuf::new(), false);
        assert!((2..=6).contains(&config.threads));
        assert!(
            config.request_timeout
                > EngineConfig::new(PathBuf::new(), PathBuf::new(), true).request_timeout
        );
    }
}
