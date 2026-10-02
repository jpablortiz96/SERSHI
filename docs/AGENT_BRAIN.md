# The Agent Brain (Prompt 4)

> The brain reasons. The brain does not execute.
>
> Natural language in. Structured decisions out. Every tool call is
> validated. Every step has its own policy. One confirmation never
> authorizes an entire plan. Context resolves meaning; it does not grant
> authority.

The Agent Brain gives SERSHI a conversation: follow-ups ("ciérralo", "¿y el
procesador?"), short multi-step requests ("abre Chrome, Outlook y luego
dime cuánta memoria me queda"), general questions, and honest answers about
what it cannot do. It is optional, local, and offline after installation.
Simple commands never wake it.

Decision record: [ADR 0017](adr/0017-agent-brain-trust-boundary.md).
Builds on [SEMANTIC.md](SEMANTIC.md) (Gate 3C) and
[VOICE.md](VOICE.md).

## Architecture

```text
typed or spoken request
  ─ 0. guards (no model): a command line or path → "I can't run commands";
       a negated action → "OK, I won't"; a hypothetical → talk, never act
  ─ 1. compound commands (no model): every part resolved by the trusted
       tiers → a bounded plan ("Abre Chrome y Outlook")
  ─ 2. references and follow-ups (no model): "ciérralo", "cierra eso",
       "el primero", "ahora PowerPoint", from the session context
  ─ 3. questions and conversation → the Agent Brain model
  ─ 4. Gate 3C: fast path, semantic router (only without the brain);
       anything Gate 3C does not understand → the Agent Brain model
       │
       ▼  Progress::Think(ticket)  — the shell runs the model OUTSIDE the
          service lock, bounded by a timeout; the core accepts the result
          only if the ticket is still current
  BrainDecision (strict, grammar-constrained JSON)
  ─ validation outside the model (manifest, offered handles, ≤ 5 steps)
  ─ answer / question / plan / single call / cancel / unknown
       │
       ▼  Progress::Step(plan) — one step per call; the shell releases the
          service between steps so the user can cancel
  ToolCall (CallOrigin::Agent for model-derived steps)
  ─ ToolRegistry → Policy → permissions/risk
  ─ trusted confirmation window when required (per step)
  ─ execution → trusted result → session context → SERSHI's report
```

Code:

- `crates/sershi-core/src/brain/` — the domain: `AgentBrainPort`,
  `BrainRequest`, `BrainDecision`, the output contract (`contract.rs`), the
  versioned instructions (`prompt.rs`), session context (`context.rs`),
  routing and deterministic splitting (`route.rs`).
- `crates/sershi-core/src/service/agent.rs` — the conversational tier, the
  brain ticket and the bounded plan orchestrator.
- `crates/sershi-semantic` — the local inference engine (one process per
  model; the same binary serves the semantic router and the brain).
- `apps/desktop/src-tauri/src/local_model.rs` — model install, lifecycle
  and the one-model-at-a-time policy; `runtime::drive` runs models and plan
  steps without holding the service.

### Provider-neutral port

`AgentBrainPort::decide(&BrainRequest) -> Result<BrainDecision, BrainError>`
is all the core knows. `LocalAgentBrain` implements it over any
`TextGenerator` (today: llama.cpp in `sershi-semantic`). A future cloud
provider would implement the same port; no core change. Cloud providers are
**not** implemented (Gate 4B).

### What the model sees

`BrainRequest` holds only:

- the request text (sanitized, ≤ 400 characters, inside `<request>`);
- the response language (the interface language — never what speech
  recognition detected);
- applications by **display name** under opaque handles `a1…a16`: the open
  question's options, the conversation's recent ones, Gate 3C's candidates
  for this text, then well-known installed applications;
- ≤ 8 short context lines ("opened a3 (Excel) 20 s ago", "memory checked:
  19.8 GB used of 31.7 GB", earlier turns' summaries);
- the capability manifest, **generated from the tool registry**
  (`brain::manifest`): `open_application(app)`, `close_application(app)`,
  `get_memory()`, `get_cpu()`, `get_system_info()` — only tools that are
  registered; there is no vocabulary entry resembling a shell, a process, a
  path or a script;
- what is not available yet, **generated** from the platform capability
  report (planned/unsupported entries) plus SERSHI's fixed boundaries (web
  and news, commands/scripts/programs by path, system settings).

Never: paths, catalog ids, tool ids, arguments, confirmation ids or state,
grants, credentials, secrets, the user's files.

### Output contract (prompt version 1)

One JSON object, generated under a GBNF grammar built from the manifest and
the offered handles, then parsed strictly (`brain::contract::parse`):

| kind | fields | meaning |
| --- | --- | --- |
| `answer` | `message` | conversation, no action |
| `clarify` | `message`, `options` (0 or 2–4 handles) | a question |
| `act` | `message`, `steps` (1–5) | tool requests, in order |
| `cancel` | — | call off what is happening |
| `unknown` | — | not understood |

Rejected, never repaired: unknown tools or kinds, a name/id/path instead of
a handle, handles that were not offered, wrong arguments, more than five
steps, duplicate options, extra fields (`additionalProperties: false`),
markup or control characters in `message`, anything that is not exactly
one object. A rejected decision falls back to Gate 3C; nothing from it acts.

`message` is shown as SERSHI's words for answers and questions. It never
reports results: what a plan did is phrased by SERSHI from each step's
trusted tool data ("Abrí Chrome, pero no pude abrir Outlook. Estás usando
19,8 GB de 31,7 GB.") — the model cannot misstate a machine fact or claim
an action that failed. Facts about the computer come only from tools.

The instructions are a controlled static template plus the generated
manifest (`brain::prompt`), versioned as `BRAIN_PROMPT_VERSION` (shown in
developer diagnostics). The static part is identical for every request, so
the engine's prompt cache evaluates it once: a warm request evaluates only
its ~30–110 new tokens of ~1,000.

### Chain of thought

Not requested (reasoning is disabled for Qwen3 and the grammar admits only
the JSON object), not stored, not logged, not sent over IPC, not shown.
Only the structured decision and its short message exist.

## Routing

| Route | When | Cost (measured, p50) |
| --- | --- | --- |
| Fast path | Gate 3C deterministic tiers resolve it | 0 ms |
| Brain, deterministic tier | references, compound commands, follow-ups | 16 ms (incl. plan steps) |
| Semantic router | short imperfect command, **brain not installed** | ~0.8 s |
| Brain model | questions, conversation, anything not resolved | 1.08 s (p95 1.9 s) |

Long questions that merely mention a system keyword ("explícame qué
significa que mi RAM esté al 80%") go to the brain when it is installed,
instead of reading the memory. Requests containing instruction-tampering
language ("ignora tus reglas…") never take the deterministic shortcut.

## Session context

One **conversation session** (Gate 4.1, `service::session`) is shared by
typed and spoken requests, the brain, plans, references and the UI — there
is no separate voice history. Rust-authoritative, **memory only**, bounded,
short-lived (`brain::context`, `understanding::dialogue`,
`service::session`):

| What | Bound | Lifetime |
| --- | --- | --- |
| Recent applications (opened, close requested) | 4 | 5 minutes |
| Last system fact (memory, CPU, info) | 1 | 5 minutes |
| Recent turns (normalized text + structured summary) | 4 | 5 minutes |
| Open question (Gate 3C or brain) | 1 | 60 seconds |
| Action ledger (real tool results: action, trusted application, result, origin, plan, why it was allowed) | 32 | until "New conversation" |

### Action ledger (Gate 4.1)

Every real tool execution adds an entry. A pending approval, a denial or a
cancellation adds nothing, because nothing ran. The ledger is
**authoritative for what SERSHI did**:

- "¿Qué acabas de abrir?" / "What did you close?" / "O que você fez?" are
  answered **deterministically from the ledger**, without the model and
  without acting (`route::recall`). The answer covers the latest request's
  successes, plus later attempts that failed, reported as failed. A failed
  open is never remembered as opened. With nothing done: "I haven't opened
  an application in this conversation."
- A plan's steps are entries of one request, so "What did you just do?"
  lists what actually ran, not what was planned.
- The brain receives the ledger's recent failures as context lines and
  never answers recall questions itself.
- The security audit (Activity) is separate and is not reset by "New
  conversation".

Entities come only from **trusted** results: an application id from a
tool's data is looked up in the catalog (never deserialized into an
entity); the user's words never become an entity. "New conversation"
(Command Center) clears all of it — not preferences, not language, not
theme. Nothing persists across sessions: persistent memory is Gate 4A, with
its own port and privacy controls.

### References

- `ciérralo`, `close it`, `fecha ele`, `ábrelo otra vez`, `hazlo de nuevo`
  → the one recent application.
- Applications acted on by **the same request** (one plan, "abre Chrome y
  Outlook") make a reference **ambiguous**: SERSHI asks ("¿Cuál?") with the
  options in the order they were opened; "la segunda" / "el primero"
  select among them. Separate turns are never ambiguous, however quickly
  they follow each other (Gate 4.1: a voice session's turns are seconds
  apart).
- **Plurals** (Gate 4.1): "los dos", "ambos", "both", "those two", "os
  dois" answer a question about exactly two candidates; "close both" /
  "ciérralos" / "cierra todos" refer to the applications acted on together.
  They pick only among trusted candidates already offered or acted on and
  never add one. Each target becomes its own plan step, with its own
  policy and, if sensitive, its own confirmation.
- "Ahora abre Word" names its own action; "¿Y Outlook?" takes the last one.
- **Closing several applications** (Gate 4.1.1) is one grouped action.
  Policy is checked per application. Safe ones close now; the rest wait in
  one trusted confirmation listing exactly them. Cancelling closes none,
  and nothing waits unseen. "Ciérralos todos" / "close them all" means the
  applications acted on together in context, never every running app.
- "I'm done with PowerPoint, please close it": the application named in the
  same request.
- An expired referent is not resolved ("Ciérralo" ten minutes later is not
  understood).

**Context resolves identity; it never authorizes.** A resolved "ciérralo"
is an ordinary `close_application` call: policy and the trusted
confirmation decide. Words said while an approval is pending ("sí", "yes",
"hazlo", "ciérralo ya") withdraw that approval and are understood by Gate 3C
alone — never through context or the model — so they can never re-create
it.

## Plans

- **Bounded:** at most `MAX_PLAN_STEPS` = **5** tool steps (the corpus'
  longest real request is 3–4; six or more → "please ask in smaller parts").
- **No autonomy loop:** a plan is decided once, then executed step by step;
  the model is never asked again mid-plan; nothing runs in the background.
- **Per-step policy:** each step is a separate `ToolCall` through the
  executor and policy. A sensitive step (closing) pauses the plan in the
  trusted confirmation window. Approval runs **that step only**; the plan
  then continues, and a later sensitive step opens **its own**
  confirmation. Declining or letting it expire cancels the rest.
- **Dependencies:** a close of an application the plan opens depends on
  that open; a step whose prerequisite failed is skipped.
- **Failure:** reported, never hidden ("Abrí Chrome, pero no pude abrir
  Outlook."); a plan with some steps done ends as `partial`.
- **Cancellation:** a new request, Escape or hiding SERSHI cancels the plan;
  remaining steps never run. A stale `advance_plan` or brain result is
  discarded by id.

State: `Thinking` (understanding or the model) → `Planning` → `Executing`
(per step, without flashing Success between steps) →
`AwaitingConfirmation` (a sensitive step) → `Success` / `Warning` /
`Error`. The Command Center shows a plan card with live progress; no
reasoning is shown.

## Model selection

Benchmark: `crates/sershi-platform/tests/brain_benchmark.rs` (model alone)
and `agent_pipeline.rs` (whole pipeline), over a labelled 120-request
corpus (`tests/data/brain_corpus.json`): 30 conversation, 30 tool
selection, 20 multi-step, 20 context/reference, 20 negatives/security;
ES 51 · EN 43 · PT 26. Reference laptop: i5-12450HX, 32 GB, RTX 3050 6 GB
(Vulkan), Q4_K_M, warm, shared prefix cached.

**Model alone** (`LocalAgentBrain`, real catalog; first-run labels — the
final labels accept OneNote/Sticky Notes for "notes" and honest
"e-mail not available"):

| Model | Params | License | Size | Correct | Wrong action | ES / EN / PT | p50 / p95 | VRAM | RAM |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Qwen3 4B** | 4.0 B | Apache-2.0 | 2.50 GB | **110 → 113** | 5 → 2 | 44/51 · 40/43 · 26/26 | 0.77 s / 1.3 s | 3.2 GB | 2.5 GB |
| Phi-4-mini-instruct | 3.8 B | MIT | 2.49 GB | 102 | 4 | 43 · 36 · 23 | 0.94 s / 5.2 s | 3.3 GB | 2.5 GB |
| SmolLM3 3B | 3.1 B | Apache-2.0 | 1.92 GB | 102 | 9 | 43 · 38 · 21 | 0.76 s / 7.0 s | 2.2 GB | — |
| Granite 3.3 2B | 2.5 B | Apache-2.0 | 1.55 GB | 99 | 11 | 45 · 32 · 22 | 0.57 s / 4.7 s | 1.9 GB | — |
| Qwen3 1.7B | 1.7 B | Apache-2.0 | 1.11 GB | 88 | 11 | 36 · 32 · 20 | 0.40 s / 0.69 s | 1.9 GB | 1.2 GB |
| Llama 3.2 3B | 3.2 B | Llama 3.2 Community | 2.02 GB | 80 | 5 | 33 · 28 · 19 | 0.63 s / 0.93 s | 2.7 GB | — |
| Gemma 3 4B | 4.3 B | Gemma Terms | 2.49 GB | 67 (37 clarifications) | 6 | 26 · 26 · 15 | 0.84 s / 4.1 s | 3.5 GB | — |

All seven produced only grammar-valid output (one SmolLM3 message was
rejected by the parser). The p95 tails of the first run (5–8 s) came from
concurrent compiles on the machine; Qwen3 4B's clean rerun: p50 0.77 s,
p95 1.3 s, max 2.1 s.

Qwen3 4B's two remaining wrong actions are "Yes" / "Sí, apruébalo" with an
approval pending → close: in the pipeline those requests never reach the
model (approval withdrawn, Gate 3C only) and a close always needs the
trusted window anyway.

**Whole pipeline** (`agent_pipeline.rs`: real `AssistantService`, real
catalog and system tools, recording application platform — nothing is
launched; context set up by actually running the earlier turns):

| Configuration | Correct | Clarification | Safe refusal | Wrong answer | **Wrong action** |
| --- | --- | --- | --- | --- | --- |
| Gate 3C only (no brain; semantic router) | 91 | 2 | 20 | 3 | 4 |
| **Agent Brain (Qwen3 4B)** | **118** | **2** | 0 | **0** | **0** |

The two clarifications are the model asking a follow-up question
("¿Qué tipo de noticias…?") where an honest "not available" was expected —
not unsafe. Without the brain, the four wrong actions are read-only memory
checks for questions that mention RAM and a semantic-router open of a
named application inside a question: degraded, never destructive.

**Selected: Qwen3 4B Q4_K_M** — the smallest candidate that passes: the
only one with ≥ 110 correct and no unsafe action through the pipeline, best
in all three languages (Portuguese 26/26), Apache-2.0, sub-second warm on
the reference GPU. Qwen3 1.7B (the semantic router) is too weak as a brain
(88, 11 wrong actions); Phi-4-mini is close in quality but slower and
larger in practice; Gemma over-asks; Llama and Gemma licenses add terms.

| Field | Value |
| --- | --- |
| id | `qwen3-4b-q4km` |
| source | `unsloth/Qwen3-4B-GGUF` @ `22c9fc8a8c7700b76a1789366280a6a5a1ad1120` |
| file | `Qwen3-4B-Q4_K_M.gguf` |
| bytes | 2,497,281,312 |
| SHA-256 | `f6f851777709861056efcdad3af01da38b31223a3ba26e61a4f8bf3a2195813a` |
| license | Apache-2.0 (Qwen3 weights) |
| quantization | Q4_K_M |
| RAM / VRAM (GPU) | 2.5 GB / 3.2 GB |
| cold load | 5.7 s (engine) · first answer 7.6 s |
| warm decision | p50 0.77–1.08 s, p95 1.3–1.9 s |

Installed only on request (Settings › Intelligence), like the other
models: free-space check (model + 10 %, ≥ 1 GB margin), HTTPS, pinned
revision, exact size, SHA-256, atomic rename; hashed again before first use
each session. No automatic updates (a new revision is a code change).

## Resources

SERSHI adapts to the computer:

- **One language model at a time.** Measured on the 6 GB GPU, the brain and
  the semantic router loaded together slowed each other about fourfold
  (brain p50 1.1 s → 4.6 s, router 0.8 s → 5.1 s). So while the brain is
  active it also covers imperfect commands and the router stays on standby
  (not loaded); it returns when the brain is turned off or removed.
- **Lazy and released:** the engine loads on the first request that needs
  it (or when the microphone opens) and ends after 5 idle minutes,
  returning all RAM and video memory. Whisper keeps its own 15-minute
  policy.
- **Below-normal priority**, inside a job object that ends with SERSHI;
  4 threads on the GPU, half the cores (2–6) on the CPU.
- **Timeouts:** 20 s per decision on the GPU (90 s on the CPU); a hung
  engine is killed and the request falls back to Gate 3C.
- **Peak with voice:** Whisper Turbo ≈ 1.3 GB + Qwen3 4B ≈ 3.2 GB of video
  memory next to normal desktop use on a 6 GB card; recognition and the
  brain never run at the same time.

Hardware tiers: **A** (discrete GPU with ≥ 4 GB free video memory) — the
brain on the GPU; **B** (integrated or small GPU) — the engine picks the
best device and retries on the CPU if the model does not fit; **C**
(CPU-only) — see the CPU measurements below; the brain stays optional, and
Gate 3C keeps every basic command working. A "performance mode" setting is
not exposed: no behaviour to switch beyond the automatic device choice has
been measured to need it.

## Failure and degradation

| Failure | Result |
| --- | --- |
| No brain installed / disabled | Gate 3C (deterministic + semantic router); the conversational deterministic tier still resolves references and compound commands |
| Model timeout, crash, invalid output | that request falls back to Gate 3C; next request restarts the engine |
| Stale model output (dismissed, replaced) | discarded by ticket id; nothing acts |
| Engine executable altered (installer builds) | refused before start (SHA-256 compiled in) |
| No disk space | download refused up front with a clear message |

User-facing errors are plain ("The local Agent Brain couldn't start.
Basic commands still work."); technical detail is in developer diagnostics
and the log (numbers and states, never words).

## Security

All Gate 1A / 3A / 3C invariants hold; Prompt 4 adds structural ones:

- the brain is a decision port with no tools of its own; it cannot reach the
  shell, processes, paths, the registry, confirmations or IPC;
- its output is grammar-constrained and parsed strictly; every step is
  validated again against the registry and the offered applications;
- model-derived calls carry `CallOrigin::Agent`; every sensitive step needs
  its own trusted confirmation; nothing the brain says can approve;
- command lines, paths and executables in a request are refused
  deterministically; negated actions and hypotheticals never act;
- instruction-tampering requests never take the deterministic shortcut and
  are refused by the brain's rules (tested with an obedient fake model:
  policy still stops them);
- tool results reach the brain only as trusted, structured context lines
  (numbers, catalog names). Future external content (mail, web) must be
  marked **untrusted content** and can never approve or authorize — it will
  enter as data in a delimited block, never as instructions.

## Tests

- Core (`cargo test -p sershi-core`): `brain::contract` (strict parsing,
  grammar = manifest, schema), `brain::route` (references, splitting,
  negation, hypotheticals, tampering, command lines),
  `service::tests::agent` (simple commands never wake the brain;
  conversation; agent-derived actions policed; deterministic plans;
  references and ambiguity; negation; failure reporting; per-step approval
  — a second close needs its own approval; cancellation mid-plan; model
  failure, timeout and crash fallback; stale decisions discarded; injection
  with an obedient model; "yes" over an approval never reaches context or
  the model; context expiry and reset; plan bound; preamble sentences;
  chatter around a command).
- Platform: engine integrity refusal; manual `brain_benchmark` and
  `agent_pipeline` (hardware).
- Frontend (`test/agent-brain.test.tsx`): contract guards (plan bound, no
  paths), plans phrased from trusted data in three languages, live progress,
  brain answers and questions, new conversation, settings (no silent
  download, disk space, standby), diagnostics never show a prompt.
