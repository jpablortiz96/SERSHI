# ADR 0016 — Semantic understanding: interpretation is not authority

**Status:** Accepted (Gate 3C)

## Context

Physical voice testing showed that understanding, not recognition, had
become the bottleneck. Examples:

- a misheard verb ("Apreer Google Chrome");
- a similar name ("Open World");
- a vendor name ("Open Microsoft Word");
- a bare name ("Outlook");
- a follow-up answer ("Windows PowerShell" after "Which one?");
- a spoken reply that left the next request refused as busy.

SERSHI needed to understand natural, imperfect requests in Spanish, English
and Portuguese. It had to keep every security invariant:

- no shell and no generic execute;
- voice, the Command Center and the companion cannot approve;
- the trusted confirmation window is the only approval surface;
- a model may request authority but never grant it.

## Decision

1. **Layered understanding, deterministic first.** Requests pass through, in
   order:
   - normalization;
   - an EN/ES/PT grammar with scoped negation;
   - the trusted catalog (exact, alias, vendor name, prefix, words);
   - fuzzy and phonetic ranking with thresholds and margins;
   - clarification;
   - and only then an optional local language model.

   A deterministic success never reaches the model.
2. **The model is a classifier behind a narrow port.** `SemanticRouterPort`
   receives the sanitized transcript, a language hint, up to 8 trusted names
   under opaque handles, the pending question's options and 3 structured
   turn summaries. It returns an enumerated intent, an offered handle or
   null, a confidence and a flag:
   - generation is grammar-constrained;
   - parsing is strict;
   - handles map back to catalog entries in SERSHI.
3. **SERSHI decides, deterministically.**
   - A model answer needs catalog evidence, and a command word matching the
     action, to act.
   - Closing is never acted on from the model or a fuzzy match.
   - Model-derived calls carry `CallOrigin::Agent`, so policy requires
     confirmation for any sensitive tool they name.
   - All calls go through the unchanged executor, policy and trusted
     confirmation.
4. **Conversation state is Rust-authoritative and ephemeral.**
   - One pending question (60 s TTL) with its trusted candidates, plus at
     most 4 recent turns, in memory only.
   - Answers select only among offered candidates.
   - A clarification answer is a new request, never an approval.
5. **`WaitingForClarification` is a first-class state**, neither busy nor
   transient. Speaking is no longer busy: a new request replaces a spoken
   reply.
6. **The model runs in its own process** (`sershi-semantic`, llama.cpp,
   Vulkan or CPU):
   - started directly, below-normal priority, inside a job object;
   - released after 5 idle minutes;
   - GGUF weights are data, installed by the same verified store as speech
     models;
   - selected by benchmark: Qwen3 1.7B Q4_K_M (Apache-2.0).

## Alternatives

- **The model in SERSHI's process.** Rejected: llama.cpp and whisper.cpp
  each statically link their own `ggml` (symbol clashes). Unloading would
  also not return all video memory, and a native crash would end SERSHI.
- **The model first, for every request.** Rejected: it adds hundreds of
  milliseconds to commands the grammar resolves in under one, and exposes
  every request to a probabilistic component.
- **The model outputs application names or tool calls.** Rejected: it would
  make the model a source of targets. Opaque handles over a trusted list
  make that impossible by construction.
- **Phi-4-mini (3.8B) as default.** Better on its own (49/54 vs 44/54), but no
  better through the policy (0 wrong actions for both), at twice the memory
  and latency and 4.5 s per request on the CPU.
- **A cloud model.** Out of scope: SERSHI is local-first and must work
  offline.

## Consequences

- Imperfect requests either act, when evidence is strong and the action is
  safe, or ask; they never guess at a destructive action.
- Understanding works fully without the model; the model only adds
  paraphrase and repair coverage.
- A new process type exists. Its binary must ship next to SERSHI; installer
  packaging is a follow-up (docs/SEMANTIC.md, known limitations).
- `ToolCall`s from understanding name catalog ids. Tools still accept names,
  so typed requests and older paths are unchanged.
- The Agent Brain (Prompt 4) can reuse:
  - `SemanticRouterPort`;
  - the dialogue state;
  - the catalog view (`CatalogNames`);
  - the confidence policy.
