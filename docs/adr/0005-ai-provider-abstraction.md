# 0005 — AI provider abstraction

**Status:** Accepted, not yet implemented (target: v0.1 LLM, v0.3 voice) · 2026-09

## Context

SERSHI must be useful without paid API keys and must never be coupled to one
vendor. Capabilities differ widely: chat/tool-calling, speech-to-text,
text-to-speech, embeddings, vision, wake word.

## Decision

- One **port per capability** in `sershi-core`, not one "AI provider" god-trait:
  `LlmProvider`, `SpeechToTextProvider`, `TextToSpeechProvider`,
  `EmbeddingProvider`, `VisionProvider`, `WakeWordProvider`.
- Adapters live in separate crates (e.g. `sershi-provider-ollama`,
  `sershi-provider-openai`) so vendor SDKs and HTTP stacks stay out of the core.
- A **model-backed `IntentResolver`** turns an `LlmProvider` into structured
  `ToolCall`s with `CallOrigin::Agent`; it replaces nothing else in the pipeline.
- **Lazy initialisation:** no provider is constructed or loaded until first use;
  local models never load at start-up.
- **Local-first default order:** Ollama (LLM), whisper.cpp / faster-whisper (STT),
  Piper (TTS). Cloud adapters are opt-in and keyed through the `CredentialStore`.

## Alternatives

- **Single `AiProvider` trait:** forces every vendor to fake capabilities it lacks.
- **LangChain-style framework:** heavy, Python/JS-centric, and hides the tool
  boundary SERSHI's security model depends on.

## Consequences

- The rule-based `KeywordIntentResolver` is the honest v0 implementation and
  remains as a fallback when no model is configured.
- Provider status becomes a `PlatformCapability`-style report shown in Settings.
- Streaming, cancellation and timeouts require the async execution runtime (v0.1).
