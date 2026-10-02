# Roadmap

Each version has one objective and explicit exit criteria. Dates are deliberately
absent; versions ship when their exit criteria are met, including Windows hardware
validation.

## v0.0.x — Foundation (current)

**Objective:** prove the architecture end to end.

- ✅ Tauri 2 shell with two windows; typed, per-window IPC
- ✅ Portable core: state machine, tools, policy, permissions, activity, intent boundary
- ✅ Three safe system tools; honest "not yet" answers for everything else
- ✅ Command Center (Home, Activity, Settings) and procedural companion
- ✅ Design tokens, motion system, reduced motion, accessibility foundations
- ✅ Contract generation, CI on Ubuntu and Windows
- ✅ Localization foundation: English, Español (Latinoamérica), Português (Brasil)
- ⏳ Manual Windows validation ([checklist](WINDOWS_PLATFORM.md#manual-validation-checklist))

### Prompt 1 — Windows Native Operator (implemented, pending Windows validation)

- ✅ Application discovery (built-ins, `shell:AppsFolder`, Start Menu, App Paths)
  with aliases and deterministic resolution ([APPLICATIONS.md](APPLICATIONS.md))
- ✅ `system.open_application` (safe) and `system.close_application` (sensitive,
  graceful `WM_CLOSE`, always confirmed)
- ✅ Trusted confirmation lifecycle owned by Rust: one-time, expiring,
  tool-matched; `AwaitingConfirmation` state and dialog
- ✅ Gate 1A — confirmation hardening: dedicated minimal-authority confirmation
  window, CSPRNG ids, `{ confirmationId, decision }` contract, single pending
  confirmation, 60 s expiry ([ADR 0011](adr/0011-dedicated-confirmation-surface.md))
- ✅ Tray (Open / Hide / Quit), global shortcut `Ctrl+Alt+Space`, single
  instance, × hides instead of quitting
- ✅ Settings → Windows integration; Developer Mode catalog inspector
- ⏳ Physical Windows validation — combined into Gate 2A below

### Prompt 2 — Visual Experience 2.0 (implemented, pending Windows validation)

- ✅ State-driven visual system: one state → visual table, one CSS renderer
  with hero / companion / compact / preview variants ([ADR 0012](adr/0012-state-driven-visual-system.md))
- ✅ Companion 2.0: distinct motion per state, hover / press / drag
  behaviour, contextual presence, companion size setting
- ✅ Command Center 2.0: core as light source, icon navigation, state glyphs,
  command acceptance, response hierarchy, activity status shapes, window
  enter / exit transitions, responsive sizing
- ✅ Appearance settings (motion, companion size), theme foundation
- ✅ Confirmation window visual polish (approval mark, amber frame and focus);
  Gate 1A behaviour unchanged
- ⏳ **Gate 2A — Windows Integrated Validation** (Prompt 1 + Gate 1A +
  Prompt 2): [checklist](WINDOWS_PLATFORM.md#gate-2a--windows-integrated-validation)
- Deferred: companion edge awareness and edge magnetism (need physical
  feedback), a "Full motion" override of the OS accessibility setting

### Gate 2B — Experience settings and personalization (implemented, pending Windows validation)

- ✅ Configurable global shortcut (Settings → Windows integration → Change):
  - Rust-validated modifier combinations
  - register-before-release; a conflict keeps the previous shortcut
  - never an automatic substitute
  - persisted
- ✅ Theme System / Light / Dark: a genuine SERSHI Light token set; System
  follows Windows live; all three windows follow the theme
- ✅ Interface sounds: an original procedural cue set (start-up, summon,
  success, error, approval request), off by default, with volume
  ([SOUND_DESIGN.md](SOUND_DESIGN.md))
- ✅ `CompanionRenderer` registry (Orbital); character companions and
  data-only packs documented ([ADR 0013](adr/0013-personalization-is-not-privilege.md))
- ⏳ Physical pass: [Gate 2B checklist](WINDOWS_PLATFORM.md#gate-2b--experience-settings-and-personalization)

### Prompt 3 — Local voice foundation (implemented, pending Gate 3A)

- ✅ Push-to-talk with native WASAPI capture:
  - memory only
  - visible in the button, the Core, the companion, the tray and Activity
  - endpointing, a 30 s cap and deterministic release
- ✅ Local speech recognition (whisper.cpp in process):
  - Automatic / English / Español / Português
  - models downloaded on request and SHA-256-verified
  - offline afterwards
- ✅ Local speech output (Windows voices), with a real Speaking state and a
  Stop button
- ✅ Real Listening / Transcribing / Speaking states and audio-reactive
  visuals
- ✅ Security: voice uses the typed pipeline and cannot approve
  ([ADR 0014](adr/0014-local-voice-foundation.md))
- ⏳ Physical pass: [Gate 3A checklist](WINDOWS_PLATFORM.md#gate-3a--local-voice-validation)

### Gate 3B — Low-latency local voice (implemented, pending physical benchmarks)

- ✅ Vulkan GPU recognition with CPU fallback; delay-loaded, System32-only
  `vulkan-1.dll` ([ADR 0015](adr/0015-low-latency-local-voice.md))
- ✅ Fast (Whisper Small q8_0) and Accurate (Large v3 Turbo q8_0) profiles
- ✅ Adaptive end of speech (600 / 900 ms), early decode during the final
  pause, warm engines (released after 15 idle minutes), background GPU
  warm-up, installed-app vocabulary context, cancellable recognition
- ✅ Developer latency diagnostics
- ⏳ Physical benchmarks with real speech:
  [Gate 3B](WINDOWS_PLATFORM.md#gate-3b--low-latency-voice)

### Gate 3C — Natural semantic understanding (accepted)

Natural language in, structured intent out, policy still decides
([SEMANTIC.md](SEMANTIC.md), [ADR 0016](adr/0016-semantic-router-trust-boundary.md)).

- ✅ Deterministic tiers first:
  - normalization;
  - EN/ES/PT grammar (wishes, fillers, scoped negation);
  - trusted aliases, vendor names and split compounds;
  - fuzzy + phonetic ranking with thresholds and margins;
  - bare names;
  - misheard command words.
- ✅ Clarification instead of guessing ("Which one?", "Did you mean …?"),
  answered by name, ordinal, yes/no or cancel, only among offered candidates.
  60 s expiry; `WaitingForClarification` state.
- ✅ Session context: Rust-authoritative, memory only, bounded.
- ✅ Busy-state fix: a spoken reply never refuses the next request.
- ✅ Catalog fix: shortcuts with different arguments are different apps
  (Windows PowerShell was hidden).
- ✅ Optional local semantic model (Qwen3 1.7B Q4_K_M, chosen by benchmark
  over Qwen3 0.6B and Phi-4-mini):
  - separate llama.cpp process, Vulkan/CPU;
  - grammar-constrained output, strict validation;
  - deterministic confidence policy;
  - verified install, lazy load, 5-minute idle release.
- ✅ Settings › Natural understanding; developer understanding diagnostics.
- ✅ Physical validation (N1 natural commands, 2026-10-01).
- Later: installer packaging of `sershi-semantic.exe`.

### Gate 3C.1 — Multilingual ASR stability (accepted)

- ✅ Interface, conversation and detected language kept separate. Replies
  never follow a misdetection.
- ✅ One contextual retry for short, unresolved utterances in an unexpected
  language. It is skipped when the first transcript is already understood.
  Whisper's language confidence is not trusted
  ([VOICE.md](VOICE.md#language-stabilization-gate-3c1)).
- ✅ Transparent "Recognized again" transcript; developer language
  diagnostics.
- Accepted by the user on physical evidence from the first 3C.1 run. The
  final contextual-retry policy is proven by automated tests; its physical
  confirmation is folded into Prompt 4's voice acceptance.

### Prompt 4 — Local Agent Brain (implemented; physical acceptance pending)

([AGENT_BRAIN.md](AGENT_BRAIN.md), [ADR 0017](adr/0017-agent-brain-trust-boundary.md))

- ✅ Provider-neutral `AgentBrainPort`; local implementation over the
  existing engine (one process per model); Qwen3 4B Q4_K_M selected by a
  120-request SERSHI benchmark (7 models).
- ✅ Routing: deterministic guards, compound commands and references first;
  the model for conversation and what the trusted tiers cannot resolve.
- ✅ Bounded plans (≤ 5 steps), per-step policy and confirmation,
  cancellation, failure reporting from trusted data.
- ✅ Ephemeral session context with expiry and "New conversation".
- ✅ Whole pipeline: 118/120 correct, 0 wrong actions; fast paths unchanged.
- ✅ One model at a time (measured), lazy load, idle release, disk-space
  check, engine integrity for installer builds, sidecar packaging overlay.
- ⏳ Physical acceptance (conversation, plans, ambiguity, negation, voice,
  security, offline, load):
  [Prompt 4](WINDOWS_PLATFORM.md#prompt-4--agent-brain)
- Next gates (not started): 4A persistent memory, 4B cloud providers,
  wake word, roles, skills, connectors.

### Gate 4.1 — Voice-first conversation (implemented; physical acceptance pending)

([ADR 0018](adr/0018-voice-session-authority.md))

- ✅ One conversation session for typed and spoken requests. An action
  ledger answers "what did you just open/close/do?" from real results.
- ✅ References by request (no time window), plural selections ("los
  dos", "both", "os dois"), "ahora abre Word".
- ✅ Voice session: explicit start, visible indicator, automatic
  turn-taking. Ends on a farewell, End, Escape, 25 s idle, unusable turns
  or 15 min.
- ✅ Half duplex with echo guard, barge-in by button, stale replies
  discarded.
- ✅ Settings › Security: "Always allow" for a closed list of low-risk
  permissions, approved in the trusted window and audited. Voice never
  approves.
- ✅ Conversation scrollback with Jump to latest; compact "New
  conversation".
- ⏳ Physical acceptance V1–V9:
  [Gate 4.1](WINDOWS_PLATFORM.md#gate-41--voice-first-conversation)
- **Why the wake word waits:** the session must first work reliably once
  SERSHI is listening. Only then is it worth deciding how SERSHI wakes up.
- Next (not started): **Gate 4.2** local conversation history · **Gate
  4.3** wake word "Hey SERSHI" · **Gate 4A** persistent personal memory.

- Next, only if Gate 3A passes: **Prompt 3B — wake word and conversational
  voice**. It covers:
  - a local wake word, opt-in, with the same Listening indicator
  - latency work: clang-cl ggml, GPU backends, streaming recognition
  - barge-in, only with echo cancellation

## v0.1 — Operator

**Objective:** the v0.1 journey works on Windows: launch → companion → "Open
Spotify" → policy → Spotify opens → success.

- ✅ Windows app launcher (`system.open_application`) and safe close (Prompt 1)
- Battery, open folder, file search in permitted folders
- ✅ Confirmation UI with one-time approvals (Prompt 1); remembered grants need
  `sershi-storage`
- `sershi-storage` (SQLite): settings, grants, persistent activity
- `CredentialStore` (Windows Credential Manager)
- First `LlmProvider` (Ollama, local) + model-backed `IntentResolver`; keyword
  resolver as fallback
- ✅ Tray icon and global shortcut (Prompt 1; `Ctrl+Alt+Space`, since `Alt+Space`
  belongs to Windows and PowerToys). ✅ Configurable shortcut and companion
  size (Gate 2B / Prompt 2). Quick Command, companion click-through and
  position settings remain
- Per-category launch policy (e.g. confirm before opening shells or security
  tools when requests are model-proposed)
- Async execution runtime with tool timeouts and cancellation

**Excludes:** voice, cloud providers other than optional OpenAI/Anthropic adapters,
skills, routines.
**Exit criteria:** journey passes the Windows checklist on two machines (one high-DPI);
idle companion < 1% CPU; no policy bypass in security tests.

## v0.2 — Context

**Objective:** SERSHI understands what the user is working on, with consent.

- Clipboard (read on request), selected-folder access, recent files
- On-demand screen capture + vision provider ("What am I looking at?")
- Prompt-injection defences for external content (delimiting, origin tracking,
  confirmation of derived actions)

**Excludes:** continuous screen monitoring. **Exit:** every context read is
user-initiated, visible and audited.

## v0.3 — Voice

**Objective:** hands-free, private voice interaction.

- ✅ Push-to-talk, VAD/endpointing, local STT (whisper.cpp), local TTS
  (Windows voices), real audio-driven visualisation. Done in Prompt 3.
- ✅ Conversation language independent of the interface language
  (automatic or chosen; see LOCALIZATION.md)
- Local wake word (Prompt 3B, after Gate 3A); neural local TTS (e.g. Piper)
- Optional cloud STT/TTS adapters (e.g. OpenAI, ElevenLabs, Google, Azure)
  through the Connector/Credential architecture

**Excludes:** voice cloning. **Exit:** microphone state always visible; no audio
retained by default; voice optional for every feature.

## v0.4 — Connected life

**Objective:** e-mail and calendar with the same safety guarantees.

- OAuth flows, Gmail/Outlook, Google/Outlook calendars
- Drafting replies; sending is sensitive (always confirmed when model-proposed)

**Exit:** tokens only in the credential store; e-mail bodies never logged.

## v0.5 — Skills

**Objective:** third parties can extend SERSHI safely.

- Skill manifest, SDK, local installation, permissions approval, disable/uninstall
- Built-in skills migrated to the same mechanism (Spotify, GitHub, VS Code)

**Excludes:** marketplace, payments. **Exit:** a skill cannot use a permission the
user did not approve (security tests).

## v0.6 — Routines

**Objective:** "When I say let's code, open VS Code, GitHub and Spotify."

- Routines with triggers (voice phrase, schedule, event), conditions, actions,
  failure policy
- Natural-language routine creation, shown as an editable plan before saving

## v0.7 — Memory

**Objective:** SERSHI remembers what the user wants it to, visibly.

- Preference and operational memory, local embeddings (`sqlite-vec`), retrieval
- Memory view: inspect, edit, delete, disable

## v0.8 — Customisation

**Objective:** SERSHI can look and sound like its user wants.

- Theme packages, companion visual packs, sound packs, voice packs
  — all **data, not code** (assets, tokens, animation metadata, procedural
  parameters), less privileged than Skills. Foundations exist since Gate 2B:
  the Light/Dark token sets, `CompanionRenderer` and `SoundSet`.
- Character companions (frame or rig based) as renderers of `AssistantState`

## v0.9 — Hardening

**Objective:** ready for non-technical users.

- Signed `SERSHI-Setup.exe`, onboarding flow, auto-update
- Crash recovery, performance budgets enforced, accessibility audit
- CODE_OF_CONDUCT, issue/PR templates, CHANGELOG (Keep a Changelog + SemVer)

## v1.0 — Stable open-source Windows desktop agent

**Exit criteria:** stable IPC and skill APIs (SemVer), documented security model
with an external review, installer for non-technical users, all v0.1–v0.9 exit
criteria still green.

## Explicitly out of scope before v1.0

Autonomous browser control, a fully autonomous computer-use agent, a skill
marketplace, payments, cloud accounts, a mobile app, production macOS/Linux
support, voice cloning, avatar marketplace.
