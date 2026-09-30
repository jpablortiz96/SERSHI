//! llama.cpp inference (Windows). One model per process.
//!
//! Performance: SERSHI's prompts share a long fixed prefix (instructions and
//! examples), so the key/value cache of the previous request is kept and
//! only the tokens after the shared prefix are evaluated — a warm request
//! evaluates tens of tokens instead of hundreds.

#![allow(unsafe_code)]

use std::num::NonZeroU32;
use std::time::Instant;

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::{LlamaModelParams, LlamaSplitMode};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::token::data::LlamaTokenData;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use llama_cpp_2::{LlamaBackendDeviceType, list_llama_ggml_backend_devices};

use crate::{Request, Response};

/// Prompt tokens evaluated per decode call.
const BATCH: usize = 512;

/// Loads `vulkan-1.dll` from System32 only (never next to the program or
/// from the working directory) before llama.cpp can reach it through the
/// delay-load helper. `false` if the machine has no Vulkan loader.
fn vulkan_runtime() -> bool {
    if !cfg!(feature = "vulkan") {
        return true;
    }
    use windows::Win32::System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW};
    use windows::core::w;
    // SAFETY: loads a system DLL by name from System32 only; the handle is
    // intentionally kept for the process lifetime.
    unsafe { LoadLibraryExW(w!("vulkan-1.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.is_ok()
}

struct Loaded {
    model: &'static LlamaModel,
    context: LlamaContext<'static>,
    /// Tokens currently in the key/value cache (sequence 0).
    cached: Vec<LlamaToken>,
}

#[derive(Default)]
pub struct Engine {
    loaded: Option<Loaded>,
}

impl Engine {
    pub fn handle(&mut self, request: Request) -> Response {
        match request {
            Request::Load {
                model,
                gpu,
                threads,
                context,
            } => self.load(&model, gpu, threads, context),
            Request::Generate {
                prompt,
                grammar,
                max_tokens,
            } => self.generate(&prompt, &grammar, max_tokens),
            Request::Shutdown => Response::default(),
        }
    }

    fn load(&mut self, path: &std::path::Path, gpu: bool, threads: u32, context: u32) -> Response {
        if self.loaded.is_some() {
            return Response::error("a model is already loaded");
        }
        if !vulkan_runtime() {
            return Response::error("vulkan loader unavailable");
        }
        let started = Instant::now();
        let Ok(mut backend) = LlamaBackend::init() else {
            return Response::error("backend init failed");
        };
        backend.void_logs();
        // The backend and model live for the whole process (one model per
        // process; unloading is ending the process).
        let backend: &'static LlamaBackend = Box::leak(Box::new(backend));

        // One device, never split across GPUs: the discrete GPU with the
        // most memory, else an integrated one, else the CPU.
        let device = gpu
            .then(|| {
                let mut devices = list_llama_ggml_backend_devices();
                devices.retain(|d| {
                    matches!(
                        d.device_type,
                        LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::IntegratedGpu
                    )
                });
                devices.sort_by_key(|d| {
                    (
                        d.device_type != LlamaBackendDeviceType::Gpu,
                        std::cmp::Reverse(d.memory_total),
                    )
                });
                devices.into_iter().next()
            })
            .flatten();
        let mut params = LlamaModelParams::default().with_split_mode(LlamaSplitMode::None);
        params = match &device {
            Some(d) => match params.with_devices(&[d.index]) {
                Ok(p) => p.with_n_gpu_layers(999),
                Err(_) => return Response::error("device selection failed"),
            },
            None => params.with_n_gpu_layers(0),
        };
        let Ok(model) = LlamaModel::load_from_file(backend, path, &params) else {
            return Response::error("model load failed");
        };
        let model: &'static LlamaModel = Box::leak(Box::new(model));
        let n_threads = i32::try_from(threads).unwrap_or(4);
        let context_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(context))
            .with_n_batch(u32::try_from(BATCH).unwrap_or(512))
            .with_n_threads(n_threads)
            .with_n_threads_batch(n_threads);
        let Ok(context) = model.new_context(backend, context_params) else {
            return Response::error("context creation failed");
        };
        self.loaded = Some(Loaded {
            model,
            context,
            cached: Vec::new(),
        });
        Response {
            ok: true,
            backend: Some(
                device
                    .as_ref()
                    .map_or("cpu", |d| d.backend.as_str())
                    .to_lowercase(),
            ),
            device: device.map(|d| d.description),
            ms: Some(millis(started)),
            ..Response::default()
        }
    }

    fn generate(&mut self, prompt: &str, grammar: &str, max_tokens: u32) -> Response {
        let Some(loaded) = self.loaded.as_mut() else {
            return Response::error("no model loaded");
        };
        let started = Instant::now();
        match run(loaded, prompt, grammar, max_tokens) {
            Ok((text, prompt_tokens, cached_tokens, generated_tokens)) => Response {
                ok: true,
                text: Some(text),
                prompt_tokens: Some(prompt_tokens),
                cached_tokens: Some(cached_tokens),
                generated_tokens: Some(generated_tokens),
                ms: Some(millis(started)),
                ..Response::default()
            },
            Err(e) => {
                // Unknown cache state after a failure: start clean next time.
                loaded.context.clear_kv_cache();
                loaded.cached.clear();
                Response::error(e)
            }
        }
    }
}

fn millis(since: Instant) -> u32 {
    u32::try_from(since.elapsed().as_millis()).unwrap_or(u32::MAX)
}

/// Greedy decoding under a grammar. The model's top choice is checked
/// against the grammar first; only if the grammar rejects it is the whole
/// vocabulary filtered (llama.cpp's own strategy: filtering ~150,000
/// candidates on every step costs more than the model itself).
fn next_token(
    context: &LlamaContext<'_>,
    grammar: &LlamaSampler,
    index: i32,
) -> Result<LlamaToken, &'static str> {
    let logits = context.get_logits_ith(index);
    let argmax = |candidates: &[LlamaTokenData]| {
        candidates
            .iter()
            .filter(|c| c.logit().is_finite())
            .max_by(|a, b| a.logit().total_cmp(&b.logit()))
            .map(LlamaTokenData::id)
    };
    let best = logits
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, l)| (i, *l))
        .ok_or("no logits")?;
    let token = LlamaToken(i32::try_from(best.0).map_err(|_| "token overflow")?);
    let mut single = LlamaTokenDataArray::new(vec![LlamaTokenData::new(token, best.1, 0.0)], false);
    single.apply_sampler(grammar);
    if argmax(&single.data).is_some() {
        return Ok(token);
    }
    let mut all = LlamaTokenDataArray::new(
        logits
            .iter()
            .enumerate()
            .map(|(i, l)| LlamaTokenData::new(LlamaToken(i32::try_from(i).unwrap_or(0)), *l, 0.0))
            .collect(),
        false,
    );
    all.apply_sampler(grammar);
    argmax(&all.data).ok_or("the grammar allows no token")
}

fn run(
    loaded: &mut Loaded,
    prompt: &str,
    grammar: &str,
    max_tokens: u32,
) -> Result<(String, u32, u32, u32), &'static str> {
    let vocab = loaded.model.vocab();
    // Special tokens in SERSHI's own template (<|im_start|> …) are parsed;
    // the user's text inside it was stripped of them by SERSHI.
    let tokens = vocab.tokenize(prompt.as_bytes(), vocab.should_add_bos(), true);
    let n_ctx = usize::try_from(loaded.context.n_ctx()).unwrap_or(0);
    if tokens.is_empty() || tokens.len() + max_tokens as usize >= n_ctx {
        return Err("prompt does not fit the context");
    }

    // Reuse the cached shared prefix; always evaluate at least one token so
    // there are fresh logits to sample from.
    let mut keep = loaded
        .cached
        .iter()
        .zip(&tokens)
        .take_while(|(a, b)| a == b)
        .count();
    if keep == tokens.len() {
        keep -= 1;
    }
    loaded
        .context
        .clear_kv_cache_seq(Some(0), u32::try_from(keep).ok(), None)
        .map_err(|_| "cache error")?;
    loaded.cached.truncate(keep);

    let mut batch = LlamaBatch::new(BATCH, 1);
    let rest = &tokens[keep..];
    for (chunk_index, chunk) in rest.chunks(BATCH).enumerate() {
        batch.clear();
        let base = keep + chunk_index * BATCH;
        let last_chunk = base + chunk.len() == tokens.len();
        for (i, token) in chunk.iter().enumerate() {
            let pos = i32::try_from(base + i).map_err(|_| "position overflow")?;
            let logits = last_chunk && i == chunk.len() - 1;
            batch
                .add(*token, pos, &[0], logits)
                .map_err(|_| "batch error")?;
        }
        loaded
            .context
            .decode(&mut batch)
            .map_err(|_| "decode failed")?;
        loaded.cached.extend_from_slice(chunk);
    }

    let mut grammar =
        LlamaSampler::grammar(loaded.model, grammar, "root").map_err(|_| "invalid grammar")?;
    let mut bytes: Vec<u8> = Vec::new();
    let mut generated = 0u32;
    let mut pos = tokens.len();
    while generated < max_tokens {
        let token = next_token(&loaded.context, &grammar, batch.n_tokens() - 1)?;
        grammar.accept(token);
        if vocab.is_eog(token) {
            break;
        }
        vocab.token_to_piece_into(token, &mut bytes, false, None);
        generated += 1;
        batch.clear();
        batch
            .add(
                token,
                i32::try_from(pos).map_err(|_| "position overflow")?,
                &[0],
                true,
            )
            .map_err(|_| "batch error")?;
        loaded
            .context
            .decode(&mut batch)
            .map_err(|_| "decode failed")?;
        loaded.cached.push(token);
        pos += 1;
    }
    let text = String::from_utf8(bytes).map_err(|_| "output is not UTF-8")?;
    let as_u32 = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Ok((text, as_u32(tokens.len()), as_u32(keep), generated))
}
