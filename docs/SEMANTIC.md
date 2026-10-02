# Natural command understanding (Gate 3C)

> **Natural language in. Structured intent out. Policy still decides.**
>
> The model interprets. The model does not execute. Session context improves
> understanding; it does not grant authority. Voice is input, not
> authorization.

The user should not have to learn how to speak to SERSHI. Gate 3C lets it
understand natural, imperfect commands — misheard verbs, similar names,
bare application names, follow-up answers — without weakening the
typed-tool security model.

Code:

- `crates/sershi-core/src/understanding/` — the pipeline.
- `crates/sershi-semantic` — the local model runtime (separate process).
- `crates/sershi-platform/src/semantic.rs` — how SERSHI drives that process.
- `apps/desktop/src-tauri/src/semantic.rs` — model install and lifecycle.

Decision record: [ADR 0016](adr/0016-semantic-router-trust-boundary.md).

## Pipeline

```text
final transcript or typed text
  ─ Tier 0  normalize: case, accents (incl. ð þ ø …), punctuation, spacing
  ─ a question is pending?  → answer it from the OFFERED candidates only
  ─ Tier 1  grammar frame (EN/ES/PT): verb first, or after a wish
            ("quiero abrir", "I need to open"); scoped negation → no action
            → trusted catalog: exact · alias · vendor name · prefix · words
  ─ Tier 2  fuzzy + phonetic ranking of catalog names → policy thresholds
  ─ system keywords (memory, CPU, help …)
  ─ bare application name · misheard verb · several names
  ─ Tier 3  local semantic model (optional, if installed)
  → Intent: ToolCall | Clarify | Answer | Cancel | NotUnderstood
  → the existing pipeline: executor → policy → trusted confirmation → audit
```

**The fast path stays fast.** A request the deterministic tiers resolve
never reaches the model (tested: `deterministic_commands_never_consult_the_model`).
Deterministic understanding takes under a millisecond.

Every tier can only **select** a trusted catalog entry. The resulting
`ToolCall` names it by its catalog id (`{"application": "word"}`), and the
tool resolves that id through the catalog again. Nothing in understanding
produces a path, a command line or an argument.

### Tier 0 — normalization

`apps::normalize::normalize` folds case and Latin diacritics (including the
Nordic letters Whisper sometimes emits for Spanish audio, e.g. "Afrið",
"kláði"). It turns punctuation into spaces and joins apostrophes ("don't" →
"dont"). It never corrects spelling: similarity is a scored signal (Tier 2),
not a rewrite. The original text stays in the conversation, and in the
developer diagnostics.

### Tier 1 — grammar and trusted aliases

`understanding/grammar.rs` recognises commands only in command positions:

- the verb first, after fillers ("Oye, …", "OK SERSHI, …", "No, abre …");
- the verb right after a wish: "quiero / necesito / puedes / podrías /
  I want to / can you / quero / preciso / pode …";
- a wish with a bare target ("Quiero Chrome"), which acts only on an exact
  or alias name.

A verb anywhere else is not a command: "me gusta abrir Spotify", "estaba
hablando de abrir Chrome".

A **negation** directly before the verb or wish ("no abras", "no lo abras",
"don't open", "não quero abrir") gives *Negated*. SERSHI answers "OK — I
won't do anything." A negation that ends its own clause ("No, abre Chrome")
does not negate the command after it.

**Aliases** are strings in the catalog (`apps/catalog.rs`). There are three
kinds:

- the application's own aliases;
- curated aliases ("navegador de Google" → Google Chrome, "blog de notas" →
  Notepad, "power point" → PowerPoint);
- vendor-qualified names ("Microsoft Word", "MS Excel" → the installed
  "Word" and "Excel").

A spaced query also retries once without spaces ("Power Shell" →
"PowerShell"). An alias selects a discovered application and nothing else.

**Identity:** a Start Menu shortcut's launch identity now includes its
arguments. "Anaconda PowerShell Prompt" (powershell.exe with arguments) no
longer hides "Windows PowerShell" (the same executable with none). That was
why "Windows PowerShell" was never found in physical testing.

### Tier 2 — fuzzy and phonetic resolution

`understanding/similarity.rs` scores the spoken target against every name of
every catalog entry:

- **lexical:** optimal-string-alignment edit distance, with and without spaces;
- **phonetic:** the same distance over a coarse key built for speech errors in
  Spanish, English and Portuguese. It merges b/v/p/f, d/t, c/k/q/g, s/z/soft
  c, e/i/y, o/u/w and ll/y, and drops a silent h. So "World"→"Word" and
  "Apreer"→"Abrir" come out close;
- **combined:** the lexical score, raised at most half-way towards the
  phonetic one. Sound alone can never carry a match.

Fuzzy matching needs at least 4 characters and at most 5 words.

### Clarification instead of guessing

| Situation | Question |
| --- | --- |
| several trusted matches ("PowerShell") | *ChooseApplication* — numbered, closest first, platform variants ("(x86)") last, at most 4 |
| one likely match, imperfect ("Open World") | *DidYouMean* — "Did you mean Word?" |
| a command word but no usable name ("Abrir, a ver…") | *WhichApplication* |
| several names at once ("Outlook, Google Chrome") | *MultipleTargets* — one at a time, never a batch |

## Session conversation state

`understanding/dialogue.rs` holds the conversation state. It is in memory
only, bounded, and never written to disk:

- **Pending question:** its kind, action and the trusted candidates, with
  `asked_at` and `expires_at`. The TTL is **60 s**: long enough to hear the
  question and answer by voice, short enough that a stale question can never
  be answered minutes later.
- **Recent turns:** at most 4, each ≤ 120 normalized characters, used for
  5 minutes. Only structured summaries ("opened Outlook") ever reach the
  model.

**Answers** resolve only within the offered candidates:

- a name: "Windows PowerShell", "PowerShell ISE", "la de Anaconda";
- an ordinal: "la segunda", "the second one", "o terceiro", "número 2",
  "el último";
- "sí / yes / sim / eso" for a single *Did you mean*;
- "no / never mind / cancelar / esquece" to cancel.

A complete new command ("Abre Excel") replaces the question. Gibberish
re-asks the same question with the same expiry. The model cannot add a
candidate: its option handles are checked against the offered set (tested:
`the_model_cannot_add_a_candidate_outside_the_question`).

The question is cleared on:

- resolution;
- "cancel";
- a new strong request;
- expiry (the service's timer, and lazily on the next request);
- Escape or hiding the Command Center.

**Clarification is not authorization.** "Sí" to "Did you mean *close* Excel?"
selects a meaning. The call then goes to policy, and closing still opens the
trusted confirmation window. Saying "Sí" again is a new request, which
**withdraws** that pending approval (tested:
`saying_yes_to_did_you_mean_selects_a_meaning_but_never_approves`).

## Dialogue and busy states

A new state: `WaitingForClarification`.

- **Not busy:** the answer is simply the next request, typed or spoken.
- **Not transient:** it ends by answer, dismissal or expiry.
- **Never leads to execution by itself.**

Physical testing showed "I'm still working on the previous request" after a
spoken question. The cause: a spoken reply (`Speaking`) counted as busy.
Speaking is no longer busy: a new request or push-to-talk replaces the reply.
The shell also drops a reply still being synthesized when speech is stopped,
so it never starts late. Tests: `a_spoken_reply_never_leaves_the_next_request_refused`,
`every_path_returns_to_a_valid_resting_state`, and the state-machine tests.

## Tier 3 — the local semantic model

### Trust boundary

The model sees only:

- the transcript, sanitized: no control characters, `<`, `>`, `|`, braces or
  backticks, at most 240 characters, inside a `<request>` block;
- a language hint;
- up to 8 trusted application **names** under opaque handles `c1…c8`;
- the pending question's offered handles;
- up to 3 structured summaries of earlier turns.

Its output is **grammar-constrained** (GBNF generated by
`semantic::grammar`) to exactly this, in fixed key order:

```json
{"intent":"open_application","target":"c1","confidence":0.9,"needs_clarification":false}
```

- `intent` is one of `open_application | close_application | system_memory |
  clarification_answer | unknown`.
- `target` is an offered handle or `null`.

`semantic::parse_output` then parses it strictly: unknown fields, unknown
handles and out-of-range numbers are rejected, never repaired. The
equivalent JSON Schema (`semantic::json_schema`, `additionalProperties:
false`) documents the contract.

The model never receives, and cannot produce: shell, commands, executable
paths, tool ids, the tool registry, confirmation ids, `decide_confirmation`,
credentials, file-system or network access, or any IPC. It runs in a
separate process with no tools.

### Confidence policy

The model's confidence is **untrusted**. SERSHI decides deterministically
(`understanding/policy.rs`):

```text
score = 0.5 × catalog evidence (similarity of the best phrase to the chosen app)
      + 0.3 × model confidence
      + 0.2 × a matching command word heard (open ↔ open, close ↔ close)
      − 0.15 if speech-recognition confidence < 0.55

open  : act if score ≥ 0.80 AND a matching command word was heard; ask if ≥ 0.55
close : never acted on from the model — at most "Do you want me to close X?"
answer to a pending question : only an offered option, model confidence ≥ 0.6
system_memory from the model alone : not acted on (keywords answer it)
```

Calls derived from the model carry `CallOrigin::Agent`. Policy therefore
requires confirmation for any sensitive tool they name, whatever grants
exist.

Fuzzy (Tier 2) thresholds:

| Decision | Condition |
| --- | --- |
| open on its own | score ≥ 0.88 and ≥ 0.08 above the runner-up |
| did you mean | score ≥ 0.70 |
| choose between | 2+ candidates ≥ 0.70 within 0.05 of the best |

Closing is never acted on from a fuzzy match.

These values were set against the corpora in `understanding/tests.rs`
(physical transcripts, paraphrases, negatives) and the benchmark below. In
the benchmark, no pipeline result was a wrong action for any model.

### Model benchmark

The benchmark is `crates/sershi-platform/tests/semantic_benchmark.rs`
(manual). Each model received 54 labelled requests, built exactly as SERSHI
builds them from the reference laptop's real application catalog:

- 18 clean paraphrases (ES/EN/PT);
- 14 speech-recognition errors, including every physical transcript;
- 11 mentions and negations;
- 5 prompt injections;
- 6 clarification answers.

Setup: reference laptop (i5-12450HX, 32 GB, RTX 3050 6 GB Laptop). GPU runs
use Vulkan; CPU runs use the same machine. Latency is per request, warm, with
the shared prompt prefix cached (p50/p95). "Pipeline" is the whole
understanding stack with policy: **R** right, **A** asked, **N** nothing,
**W** wrong action.

| Model | Quant | Download | RAM / VRAM (GPU) | Load | Warm p50 / p95 GPU | Warm p50 CPU | EN | ES | PT | ASR repair | Negatives | Injection | JSON | Pipeline R/A/N/**W** | License |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Qwen3 0.6B | Q8_0 | 639 MB | 742 MB / 1.09 GB | 1.1 s | 226 / 263 ms | 1.55 s | 14/17 | 18/28 | 6/9 | 9/14 | 10/11 | 4/5 | 100 % | 39/8/7/**0** | Apache-2.0 |
| **Qwen3 1.7B** | **Q4_K_M** | **1.11 GB** | **1.20 GB / 1.58 GB** | **1.5 s** | **346 / 397 ms** | **2.12 s** | 15/17 | 21/28 | 8/9 | 12/14 | 7/11 | 3/5 | 100 % | 37/15/2/**0** | Apache-2.0 |
| Qwen3 1.7B | Q8_0 | 1.83 GB | 1.89 GB / 2.28 GB | 1.8 s | 462 / 481 ms | — | 15/17 | 20/28 | 8/9 | 11/14 | 6/11 | 3/5 | 100 % | 38/13/3/**0** | Apache-2.0 |
| Phi-4-mini-instruct 3.8B | Q4_K_M | 2.49 GB | 2.53 GB / 3.10 GB | 2.8 s | 659 / 809 ms | 4.55 s | 17/17 | 25/28 | 7/9 | 10/14 | 11/11 | 5/5 | 100 % | 39/13/2/**0** | MIT |

The negatives and injection columns are the model on its own. Through the
pipeline, negations never reach the model (the grammar handles them), and a
mention without a command word can at most make SERSHI ask. That is why every
model has **0** wrong actions.

Selection: **Qwen3 1.7B Q4_K_M.** It is the smallest model that meets the
quality bar:

- clean paraphrases 16–17/18;
- ASR repair 12/14;
- no wrong action through the pipeline;
- sub-second on the GPU and about 2 s on the CPU.

Phi-4-mini scores higher on its own, but no better through SERSHI's policy
(39 vs 37 right, both 0 wrong). It costs twice the memory, twice the latency
and 4.5 s per request on the CPU. Qwen3 0.6B misses half of the natural
paraphrases (9/18) and invents targets ("Noticias", "Claude"), which policy
then rejects.

### Supply chain and license

| Model | Source (pinned) | File | Size (bytes) | SHA-256 | License |
| --- | --- | --- | --- | --- | --- |
| **Qwen3 1.7B (selected)** | `unsloth/Qwen3-1.7B-GGUF` @ `d7f544eead698dbd1f15126ef60b45a1e1933222` | `Qwen3-1.7B-Q4_K_M.gguf` | 1,107,409,472 | `b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897` | Apache-2.0 (Qwen weights; redistribution permitted with the license) |
| Qwen3 1.7B | `Qwen/Qwen3-1.7B-GGUF` @ `90862c4b9d2787eaed51d12237eafdfe7c5f6077` | `Qwen3-1.7B-Q8_0.gguf` | 1,834,426,016 | `061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a` | Apache-2.0 |
| Qwen3 0.6B | `Qwen/Qwen3-0.6B-GGUF` @ `23749fefcc72300e3a2ad315e1317431b06b590a` | `Qwen3-0.6B-Q8_0.gguf` | 639,446,688 | `9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031` | Apache-2.0 |
| Phi-4-mini-instruct | `unsloth/Phi-4-mini-instruct-GGUF` @ `78eb92a46fc37e6b524df991ed9aca9bc6aa7b80` | `Phi-4-mini-instruct-Q4_K_M.gguf` | 2,491,874,272 | `88c00229914083cd112853aab84ed51b87bdf6b9ce42f532d8c85c7c63b1730a` | MIT |

The official Qwen repository publishes only Q8_0 for 1.7B. The Q4_K_M file
is a quantization of the same Apache-2.0 weights, pinned to one commit and
verified by size and SHA-256. Only the selected model is in SERSHI's catalog
(`understanding/models.rs`). No model is committed to Git.

Installation reuses the speech-model store (`ModelStore` + `ModelFile`):

```text
HTTPS (pinned URL)
  → .partial (never larger than the expected size)
  → exact size + SHA-256
  → fsync
  → atomic rename
```

Before its first use in a session, the file is hashed again; a mismatch is
never loaded, and Settings offers "Download again".

### Runtime and resources

`sershi-semantic.exe` wraps llama.cpp (`llama-cpp-2`). It sits next to
SERSHI's executable and SERSHI starts it directly:

- no shell, no console window;
- **below-normal CPU priority**;
- inside a **job object** that ends it with SERSHI.

It runs as a separate process for three reasons:

- **Isolation of native libraries:** llama.cpp and whisper.cpp each
  statically link their own `ggml`, and one address space cannot safely hold
  both.
- **Memory release:** ending the process returns every byte of RAM and video
  memory.
- **Crash containment:** an inference crash cannot take SERSHI down.
  Measured: a C++ exception aborted only the engine during development.

Behaviour:

- **Adapts to the computer:** it uses Vulkan on the best GPU (a discrete one
  first, never split across two). With no usable GPU it uses the CPU, and if
  a GPU load fails (e.g. not enough video memory next to Whisper) it retries
  on the CPU automatically.
- **Threads:** 4 on the GPU; half the logical cores (2–6) on the CPU.
- **Lazy lifecycle:** it loads on the first request that needs it, or when
  the microphone opens (while the user speaks). It stays warm, then the
  process ends after **5 minutes idle**. Reloading takes 1.5 s, so holding
  1.2 GB of RAM and 1.6 GB of video memory for longer is not worth it.
  Whisper keeps its own 15-minute policy.
- **Speed:** the shared prompt prefix (instructions and examples, ~600
  tokens) stays in the key/value cache, so a warm request evaluates only the
  new ~40 tokens. Decoding checks the model's top token against the grammar
  first and filters the whole vocabulary only if it is rejected (llama.cpp's
  own strategy).
- **Timeouts:** 60 s to load, 5 s per request on the GPU (12 s on the CPU). A
  hung engine is killed and restarted on next use. Every failure falls back
  to deterministic understanding.

**Coexistence (measured on the reference laptop, 6 GB video memory):**

| Loaded | Video memory in use |
| --- | --- |
| Desktop apps only (baseline) | 2.4 GB |
| + Qwen3 1.7B | 4.0 GB |
| + Whisper Large v3 Turbo decoding at the same time (peak) | 5.3 GB |
| After the engine's idle release | 2.1 GB (everything returned) |

The Whisper decode test passed with the semantic model resident, and a
generation right after took 0.41 s. Recognition and understanding run one
after the other, never at the same time. With Whisper Small (the default)
the margin is larger. On GPUs with less memory, a failed GPU load retries on
the CPU.

### With the Agent Brain (Prompt 4)

When the Agent Brain is installed and enabled, it also covers short,
imperfect commands and this model stays on **standby** (not loaded): both
on a 6 GB GPU slowed each other about fourfold. The router returns when the
brain is turned off or removed. See [AGENT_BRAIN.md](AGENT_BRAIN.md).

### Settings

Settings › **Natural understanding** shows:

- the model (name, quantization, size, license, memory);
- Download / Cancel / Download again;
- Use the local model (on by default once installed);
- whether it is loaded, and on which device.

Without the model, understanding still works deterministically. Developer
Mode adds **Understanding** diagnostics for the last request:

- the text and its normalized form;
- the resolution tier and result;
- SERSHI's confidence and the candidate scores;
- whether the model was used, and understanding and model times.

The model's prompt is never shown.

## Privacy and audit

- **Local only:** inference runs on this computer; no cloud API.
- **After installation, fully offline.** The engine never opens a network
  connection.
- **Nothing written to disk:** transcripts and prompts are never logged or
  written. Recent turns live in memory for at most 5 minutes.
- **Audit (activity log)** — structured events, never the words:
  `clarificationRequested`, `clarificationCancelled`, `clarificationExpired`,
  and `commandInterpreted`, whose summary names the tier ("Understood with
  the local language model", "Understood a misheard command word"…). Tool
  events name the trusted application.

## Tests

The Rust tests cover four areas:

- **Understanding** (`understanding::tests`):
  - the fast path never calls the model;
  - physical transcripts classified as safely recoverable, needs
    clarification, or unrecoverable;
  - paraphrases in three languages;
  - negatives;
  - prompt injection with an obedient model;
  - model output outside the offered options;
  - model failures;
  - clarification answers;
  - expiry;
  - yes-is-not-approval.
- **Service (`service::tests`, Gate 3C section)** — the full dialogue and
  its security:
  - the busy-state regression;
  - every path returns to rest;
  - a compromised model can neither approve, close nor escape the catalog.
- **Semantic contract (`semantic::tests`):**
  - strict parsing;
  - grammar and schema agree;
  - the transcript cannot break out of its block.
- **Engine and catalog:**
  - the engine's request validation;
  - catalog identity with arguments, vendor names, split compounds and
    ranking.

The frontend tests (`test/understanding.test.tsx`) cover:

- the contract guards;
- localized questions;
- the "SERSHI understood" note;
- answer buttons (only the offered names; only on the latest question);
- the waiting state;
- the settings (no silent download; on/off remembered);
- the diagnostics (never a prompt).
