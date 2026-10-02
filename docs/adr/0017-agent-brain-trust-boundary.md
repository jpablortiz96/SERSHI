# ADR 0017 — The Agent Brain: a decision port, bounded plans, per-step authority

**Status:** Accepted (Prompt 4)

## Context

SERSHI understood commands (Gate 3C) but not a conversation: follow-ups
("ciérralo"), several actions in one request, general questions. Prompt 4
adds a local language model for that — the most capable component SERSHI
has run so far, and the one with the most room to be wrong or manipulated.
Every invariant must hold: no shell, no generic execute, the trusted
confirmation window as the only approval surface, voice and the Command
Center unable to approve, and a model that may request but never grant
authority.

## Decision

1. **The brain is a decision port, not an executor.** `AgentBrainPort`
   returns a closed `BrainDecision` (answer, clarify, act, cancel, unknown).
   It has no tools, no IPC, no paths. Execution stays with the
   `ToolRegistry`, the executor, policy and the confirmation store — the
   same path as a typed command. Model-derived calls carry
   `CallOrigin::Agent`.
2. **Structured output, validated outside the model.** Generation is
   constrained by a grammar built from the capability manifest (generated
   from the registry) and the offered application handles; the parser
   rejects anything unexpected; the service validates each step again.
   Invalid output is never repaired into an action.
3. **Deterministic first.** Guards (command lines, negations,
   hypotheticals), compound commands and references resolve without the
   model; trivial commands never wake it. The model takes conversation,
   questions and what the trusted tiers cannot resolve.
4. **Bounded plans, per-step authority.** At most five steps; decided once,
   executed one step per call; each step through policy; a sensitive step
   pauses for its own confirmation; approval covers that step only; decline
   or expiry cancels the rest. No loop asks the model again; nothing runs in
   the background.
5. **The model runs outside the service lock.** A ticket ties a decision to
   its request; a cancelled, dismissed or replaced request's decision is
   discarded. Timeouts bound inference; failures fall back to Gate 3C.
6. **Ephemeral, Rust-authoritative context.** Recent applications, one
   system fact and a few turns, memory only, minutes-long, cleared by "New
   conversation"; entities only from trusted results. Context resolves
   identity, never authority; words said over a pending approval never
   reach context or the model.
7. **Results are reported from trusted data.** The model's message is shown
   for answers and questions; what a plan did is phrased by SERSHI from tool
   results.
8. **No chain of thought** is requested, stored, logged, sent or shown.
9. **Local, one model at a time.** Selected by a SERSHI-specific
   benchmark: Qwen3 4B Q4_K_M (Apache-2.0). The same engine binary as Gate
   3C, one process per model; while the brain is active the semantic router
   is on standby (measured fourfold slowdown when both share a 6 GB GPU).
10. **Cloud providers deferred** to Gate 4B; the port is provider-neutral.

## Alternatives

- **The model executes tool calls directly (function calling into the
  registry).** Rejected: it would make the model a source of authority;
  validation must sit outside it, and steps must meet policy one by one.
- **An autonomous loop** (plan, act, observe, re-plan until done).
  Rejected for now: unbounded, hard to cancel, and every iteration is a new
  chance for injected text to steer actions. A bounded, single-decision
  plan covers the measured requests.
- **One confirmation for a whole plan.** Rejected: approving "open Chrome
  then close Excel" must not approve the close implicitly, and a plan's
  later steps can change meaning (a step failed, an app closed).
- **Model output phrasing results.** Rejected: the model could misstate
  machine facts or claim a failed step succeeded.
- **A separate brain executable.** Rejected: the existing engine already
  isolates one model per process; a second binary would duplicate build,
  packaging and integrity work. The same binary runs two processes when
  needed.
- **Brain + semantic router resident together.** Rejected by measurement on
  6 GB GPUs (both ~4× slower).
- **Persistent conversation memory.** Deferred to Gate 4A with explicit
  privacy controls and a separate port.

## Consequences

- Conversation, follow-ups and short plans work locally, with no wrong
  action through the pipeline on the 120-request corpus.
- Simple commands keep their cost (0 ms understanding); conversational
  requests cost ~1 s on a mid-range GPU; CPU-only machines are slower and
  the brain stays optional.
- A model release now ships a 2.5 GB optional download; installer builds
  bundle and hash the engine executable.
- The decision contract (`BRAIN_PROMPT_VERSION`) is versioned; behaviour
  changes are auditable in diagnostics and tests.
- Future connectors must deliver external content as untrusted data that can
  never approve or authorize.
