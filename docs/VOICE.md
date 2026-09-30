# Voice

SERSHI can hear you and answer out loud, entirely on your computer.

> **Voice is input. Voice is output. Voice is not authorization.**
> Speaking can ask SERSHI for anything you could type. It can never approve
> an action, change a permission or grant a credential.

Status: Prompt 3 (push-to-talk foundation). It is implemented and tested on
the reference Windows machine; physical sign-off is Gate 3A (below). The wake
word is not implemented.

## For everyone: what happens to your voice

| Question | Answer |
| --- | --- |
| When is the microphone on? | Only after you press the microphone button in the Command Center, until you press it again, stop talking for about a second, press Escape, type a command or hide the Command Center. It is never on at start-up and never in the background. At most 30 seconds per press. |
| How do I know it is on? | The microphone button fills and pulses. The Core and the companion show **Listening**, the tray tooltip says "SERSHI — Listening", and Activity records "Microphone on" / "Microphone off" (with the duration). Windows also shows its own microphone indicator in the taskbar. |
| Where does the audio go? | Nowhere. It stays in SERSHI's memory for one utterance, is turned into text on this computer, and is then discarded. |
| Is anything saved? | No recordings, ever: not on disk, not in logs, not in settings or history. The words SERSHI understood appear in the conversation ("You said …"), which is kept in memory for this session only, like typed text. |
| Does it need the internet or an account? | Only once, to download the speech model when you choose to. After that, voice works offline. No API keys, no cloud. |
| What gets downloaded, and from where? | One speech model file (264 MB by default) from the whisper.cpp project on Hugging Face, over HTTPS. SERSHI checks its exact size and SHA-256 fingerprint before using it. It is data; it is never run as a program. |
| Where are models stored? | `%LOCALAPPDATA%\dev.sershi.desktop\models\stt\`. Delete a file there to remove a model. |
| Can voice approve something? | No. Sensitive actions (like closing an application) still open the separate confirmation window, and only a click there approves. Saying "yes" or "sí" does nothing. Pressing the microphone while a confirmation is open cancels it. |
| How do I turn voice off? | Just don't press the microphone: nothing listens otherwise. Spoken replies can be turned off in Settings › Voice › Voice responses. To remove voice entirely, delete the model file. |
| Can a video playing on my speakers command SERSHI? | Not in normal use. The microphone is off unless you pressed it, and anything heard still goes through the same rules and confirmations as typed text. |

## Architecture

```text
Command Center (React)                       Rust (Tauri shell + core)
──────────────────────                       ─────────────────────────────────────────
[mic] click ── start_voice_capture ────────► voice::start_capture
                                               core: begin_listening
                                               (cancels a pending approval) → Listening
                                               WASAPI capture thread (cpal) ──► channel
                                               session thread: downmix → endpoint (VAD)
                                                 → level 0–1 ── sershi://voice-level ─► Core/companion
                                               mic released → core: end_listening → Transcribing
                                               resample 48k→16k → whisper.cpp (in process)
                                               transcript::assess (no speech / unclear / accept)
"You said …"  ◄── sershi://voice {heard} ───── accepted text
                                               core: submit_transcript(text)
                                                 == submit(text): intent → ToolCall → policy
                                                 → (confirmation window if sensitive)
reply phrased ◄── sershi://voice {answered} ── CommandOutcome (never a confirmation id)
in UI language ── speak_reply(text, locale) ─► Windows SpeechSynthesizer → PCM in memory
                                               core: begin_speaking → Speaking
                                               WASAPI playback (cpal) → level ─► Core/companion
                                               playback ends → core: end_speaking → Ready
```

| Layer | Where | What |
| --- | --- | --- |
| Domain (portable) | `crates/sershi-core/src/voice/` | Ports, language tags, resampling, level meter, endpoint detector, transcript assessment, model catalog, IPC payloads |
| State machine | `sershi-core::assistant` | `Listening`, `Transcribing`, `Speaking` and their legal transitions |
| Service | `sershi-core::service` | `begin_listening`, `end_listening`, `submit_transcript` (→ `submit`), `voice_unusable`, `cancel_voice`, `voice_failed`, `begin_speaking`, `end_speaking` |
| Windows adapters | `crates/sershi-platform/src/windows/voice/` | WASAPI capture and playback (`cpal`), whisper.cpp (`whisper-rs`), WinRT speech synthesis, WinHTTP download, microphone privacy check |
| Model store (portable) | `crates/sershi-platform/src/voice/model_store.rs` | Verified, atomic model installation |
| Orchestration | `apps/desktop/src-tauri/src/voice.rs` | Threads, channels, events, commands |
| UI | `apps/desktop/src/state/voice.ts`, `components/command/`, `surfaces/command-center/VoiceSettings.tsx` | Push-to-talk, notices, Settings › Voice |

### IPC (Command Center only)

| Command | Purpose |
| --- | --- |
| `get_voice_status` | Microphones, privacy switch, voices, models, capturing/speaking |
| `configure_voice` | Device, conversation language, voice and model ids (validated) |
| `start_voice_capture` | Push-to-talk; returns `started` or `refused {reason}` |
| `stop_voice_capture` / `cancel_voice_capture` | Stop and transcribe / discard |
| `speak_reply` / `stop_speaking` | Speak an already-phrased reply (≤ 600 chars) / stop |
| `download_voice_model` / `cancel_voice_model_download` | User-initiated model download |

Events:

- `sershi://voice` (heard, answered, noSpeech, unclear, cancelled, failed,
  deviceFallback): main window only.
- `sershi://voice-level` (`{ level: 0–1, source }`, ≤ 25/s): main window
  and companion.
- `sershi://voice-model` (download progress): main window only.

No command or event carries audio.

### States

```text
Idle/Awake/outcome ─StartListening→ Listening ─CaptureEnded→ Transcribing
Transcribing ─RequestReceived→ Thinking → Planning → Executing → Success/Warning/Error
Transcribing ─AttentionNeeded→ Warning        (no speech, unclear: nothing runs)
Listening/Transcribing ─Dismiss→ Idle         (Escape, hide, typed command)
Listening/Transcribing ─Failed→ Error         (device lost, model damaged)
Success/Warning/Error/Idle ─SpeechStarted→ Speaking ─SpeechEnded→ Idle
```

- `StartListening` is refused while working, speaking or already in voice.
  From `AwaitingConfirmation`, the service first cancels the approval and
  returns to Idle.
- `SpeechStarted` never replaces `AwaitingConfirmation`: the reply still
  plays, but the approval state stays visible.
- Voice states cannot lead to `ExecutionStarted` or `ConfirmationApproved`
  (tested exhaustively in `assistant.rs`).

## Security: audio does not grant authority

1. **Same pipeline.** `submit_transcript` checks the state is
   `Transcribing` and then calls `submit`, the function behind
   `submit_command`. Test: `a_voice_request_runs_exactly_the_typed_pipeline`
   compares a spoken and a typed "Abre Spotify": identical outcome and audit
   trail, plus the microphone entries.
2. **No approval intent.** `Intent` has no approve variant. Test:
   `approval_words_are_never_an_intent` covers "yes", "sí", "sim", "ok",
   "approve", "aprobar", "confirm", "confirmar", "hazlo", "SERSHI, yes"…
3. **No overlap.** Starting to listen cancels a pending confirmation, so
   nothing said can land while an approval is open. Tests:
   `the_microphone_never_overlaps_a_pending_approval` and
   `speaking_yes_never_approves_a_sensitive_action` (13 phrasings; nothing
   closes, and even the trusted surface can no longer approve the cancelled
   id).
4. **Cancel is safe.** Exact utterances ("Cancel", "Cancelar", "Cancelar
   ação"…) withdraw a pending approval. Cancellation only reduces authority.
5. **Capabilities.** Voice commands are granted to the Command Center only.
   The companion's and the confirmation window's capabilities are unchanged
   (contract tests "Gate 3A"). The confirmation bundle contains no voice code
   (`bootstrap.tsx` imports the low-level IPC client only; test in
   `voice.test.tsx`).
6. **No generic execution.** No process is spawned, no shell is used, and
   the model URL is fixed per catalog entry. The download accepts
   `https://` only, TLS 1.2+, and refuses HTTPS→HTTP redirects.
7. **Untrusted text.** Transcripts are filtered for silence, recogniser
   hallucinations and low confidence ("I couldn't understand that clearly"
   rather than a guessed command), then treated like typed text.

## Privacy and logging

- **Memory only.** Audio lives in the session thread's buffer and is dropped
  after recognition. Synthesized speech is decoded in memory.
- **Operational logs** (stderr) contain only:
  - capture duration
  - recognition time and model id
  - speech start latency
  - model load time
  - generic failure codes
  They never contain transcripts, audio or buffers. whisper.cpp's own
  logging is disabled.
- **Activity.** "Microphone on" and "Microphone off (duration)". Never what
  was said.
- `Transcript`'s `Debug` output prints only the character count.

## Endpointing (VAD)

Endpointing is UX only, never a security decision. It is an energy detector
over 20 ms frames with an adaptive noise floor:

- The first 100 ms seed the floor, capped at −45 dBFS so someone who talks
  immediately is still heard.
- The floor falls fast, rises slowly in pauses, and barely rises during
  speech.
- Speech means more than 10 dB over the floor and above −50 dBFS, for at
  least 200 ms in total.
- **Adaptive end of speech (Gate 3B):**
  - Less than 1.5 s of speech ("Abre Chrome") ends after 600 ms of silence.
  - Longer requests end after 900 ms, so thinking pauses don't cut them.
  - Prompt 3 used a fixed 1.0 s.
  - Developer Mode can pin 450 / 550 / 650 / 750 ms for tuning.
- With no speech after 8 s, the result is "No speech detected".
- The hard cap is 30 s; SERSHI then transcribes what it heard.
- A stream of exact zeros is reported as a silent microphone. Windows
  privacy settings do this to desktop apps.

The detector is tested with generated audio (tones, syllable-modulated
"speech", deterministic noise); no recordings are committed.

## Speech recognition

The engine is **whisper.cpp** through `whisper-rs` 0.16 (bindings
Unlicense, whisper.cpp MIT), in process. Decoding is tuned for short
commands, with no measured accuracy cost:

- greedy decoding, one candidate
- temperature 0
- single segment
- no timestamps
- non-speech tokens suppressed

### Acceleration

| Where | When | Threads |
| --- | --- | --- |
| **GPU through Vulkan** | A Vulkan GPU driver is present (NVIDIA, AMD or Intel) | ≤ 4 CPU threads |
| **CPU** | Always available; also the fallback when a GPU load fails, e.g. not enough video memory | min(6, logical cores / 2) |

The CPU thread count is measured: on the 12-thread reference CPU, 6 threads
took 5.0–5.9 s, while 10–12 threads were slower (8.7–10.7 s) and starved
the UI.

How the GPU is used safely:

- SERSHI is built with whisper.cpp's Vulkan backend. `vulkan-1.dll` is
  **delay-loaded** and loaded explicitly from **System32 only**, so a
  planted copy next to SERSHI is never used.
- On a machine without the Vulkan loader, whisper.cpp is never called,
  because its backend registry enumerates Vulkan devices on first use.
  Voice then reports itself unavailable instead of crashing.
- The device used is the first GPU ggml reports (the RTX 3050, ahead of the
  Intel UHD, on the reference laptop). Settings › Voice › Speech
  acceleration shows it.
- CUDA and OpenVINO were not adopted:
  - CUDA needs a 3 GB toolkit to build and ships hundreds of MB of DLLs.
  - OpenVINO isn't supported by the current dependency stack.
  - Vulkan is cross-vendor and needs nothing extra on users' machines.
- A clang-cl build of whisper.cpp gave no CPU gain over MSVC (6.1–6.6 s vs
  5.4–6.8 s), so the build stays on MSVC.

**Warm-up.** A GPU's first use compiles shader pipelines: about 25 s the
first time a program runs on a machine (the driver caches them afterwards).
The first Automatic decode and the first decode with a vocabulary context
also compiled extra pipelines. So after a GPU load, SERSHI decodes one
second of silence three ways (fixed language, detection, context) in the
background. Warm-up:

- runs after a model is installed, and while the user speaks the first
  command after start-up;
- never opens the microphone, runs a command or keeps a result.

**Warm engines.** A loaded engine stays in memory between commands. Engines
are released after 15 minutes without voice use, and when the profile
changes.

### Language

- **Automatic**: Whisper detects the language on **every** utterance.
  Reusing the last language as a hint was evaluated and rejected: with a
  wrong hint Whisper *translates*. English speech with a Spanish hint came
  out as "¿Cómo mucho memory estoy usando?".
- **English / Español / Português**: passed straight to Whisper. No
  detection runs, which is faster. Settings says so.

The interface language never changes because of what you said.

### Faster end of speech

- **Early decode.** When the user pauses for 200 ms, SERSHI starts decoding
  what it has, overlapping the end-of-speech wait. If the endpoint then
  fires with no new speech, that result is used. If the user speaks again,
  the early decode is aborted and the full utterance is decoded.
- **Vocabulary context.** Up to 12 well-known *installed* application names
  from the trusted catalog (e.g. "Chrome, Outlook, Notepad.") are given to
  Whisper to help spelling. This is recognition context only: it never
  selects, launches or authorizes anything (tested). Its measured effect is
  small and mixed:
  - Small: "Hables" → "Abres" (which the resolver accepts).
  - Turbo: "Hable" → "Habla", with lower confidence.
  - It costs about 0.05–0.1 s, and never leaked into silence.
- **Second opinion.** A Fast result too unclear to act on is re-decoded
  once by the Accurate model, only if that model is installed and a GPU
  makes it quick. Otherwise SERSHI says it couldn't understand. It never
  guesses a command.

Security is unchanged: early, partial and warm-up text is **never
submitted**. Only the final transcript, after capture ends, goes to
`AssistantService::submit_transcript` (the typed path). A recognition the
user cancels is aborted and its result discarded (tested).

### Models

Source: `huggingface.co/ggerganov/whisper.cpp`, pinned revision
`5359861c739e955e79d9a303bcbc70fb988958b1`. These are GGML conversions of
OpenAI Whisper (weights: MIT).

| | Fast (default) | Accurate |
| --- | --- | --- |
| Model id | `whisper-small-q8` | `whisper-large-v3-turbo-q8` |
| File | `ggml-small-q8_0.bin` | `ggml-large-v3-turbo-q8_0.bin` |
| Quantization | q8_0 | q8_0 |
| Exact size | 264,464,607 bytes (≈264 MB) | 874,188,075 bytes (≈874 MB) |
| SHA-256 | `49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f` | `317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1` |
| URL | `https://huggingface.co/ggerganov/whisper.cpp/resolve/<revision>/ggml-small-q8_0.bin` | `…/ggml-large-v3-turbo-q8_0.bin` |
| License | MIT | MIT |
| Memory (measured) | Vulkan: +105 MB RAM (weights in VRAM); CPU: +271 MB RAM | Vulkan: +50 MB RAM (weights in VRAM); CPU: +880 MB RAM |
| Recognition, GPU (RTX 3050 Laptop), fixed language | 0.36–0.41 s | 0.82–0.83 s |
| Recognition, GPU, Automatic | 0.45–0.9 s | 1.35–1.47 s |
| Recognition, CPU, fixed / Automatic | 5.7–6.1 s / 8.8–10.6 s | 38–40 s / 64–82 s |

The recognition times are for a 1.5 s utterance.

**Dropped in Gate 3B:**

- **Whisper Base q8_0** (`ggml-base-q8_0.bin`, 81,768,585 bytes). This was
  the ~81 MB file in the Prompt 3 download test. It mis-heard Spanish
  commands, once as Greek with 0.89 confidence.
- **Large v3 Turbo q5_0** (574 MB). q8_0 is faster on the GPU (1.5 s vs
  2.1–2.5 s on Automatic) with the same accuracy.

Neither is offered any more.

**Profiles.**

- **Fast** is the default: near-instant with a GPU and usable on CPU.
- **Accurate** is for dictation and long questions. Without a GPU it is
  impractical (tens of seconds), and Settings says so.

**Download security** (unchanged, for every model):

1. The user clicks Download (the size is shown first).
2. `https://` with a pinned URL.
3. The body is streamed to `<file>.partial`, never beyond the expected size.
4. Exact size and SHA-256 must match; otherwise the file is deleted.
5. `fsync`, then an atomic rename.
6. Before the first load in each session, the file is hashed again.
7. A damaged file is reported and never loaded.
8. Leftover `.partial` files are removed at start-up.

## Speech output

**Windows `SpeechSynthesizer`** (OneCore voices from Windows language packs)
synthesizes plain text to an in-memory WAV. SERSHI resamples it to the
output device and plays it through WASAPI, so:

- **Speaking** lasts exactly as long as playback.
- The companion follows the real output level.
- **Stop speaking** (button, Escape, a new request, the microphone) stops
  it at once.

Voice selection:

1. the voice chosen in Settings;
2. otherwise the system default voice, if it speaks the reply's language;
3. otherwise the first installed voice for that language;
4. otherwise the system default voice.

Only installed voices are listed.

Speaking rules:

- Replies to spoken requests are spoken (Settings › Voice › Voice responses,
  on by default).
- Typed requests stay silent unless "Speak typed responses" is on.

**Interface sounds vs speech.** They are independent settings. No UI cue
plays while listening, transcribing or speaking. The success/error cue is
skipped when a spoken reply will say it. The approval cue always plays.

## Building

Windows builds need, in addition to [WINDOWS_PLATFORM.md](WINDOWS_PLATFORM.md)'s
prerequisites (Visual Studio 2022 Build Tools, Rust, Node, pnpm):

| Tool | Why | Install |
| --- | --- | --- |
| **CMake** | builds whisper.cpp | ships with VS Build Tools: add `…\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin` to `PATH` (or install CMake) |
| **Ninja** | CMake generator for whisper.cpp; MSBuild's file tracker fails on the Vulkan backend's nested build paths | ships with VS Build Tools: `…\CommonExtensions\Microsoft\CMake\Ninja` on `PATH` (or `pip install ninja`) |
| **LLVM** (libclang) | `bindgen`: whisper-rs-sys's bundled bindings are generated on Linux and do not match MSVC's type sizes | `winget install LLVM.LLVM`; set `LIBCLANG_PATH=C:\Program Files\LLVM\bin` if it isn't found |
| **Vulkan SDK** 1.4.357.0 | GPU backend: headers, `vulkan-1.lib`, `glslc` | `winget install KhronosGroup.VulkanSDK`; sets `VULKAN_SDK` |

Without the Vulkan SDK, build a CPU-only SERSHI with
`--no-default-features` on `sershi-desktop` / `sershi-platform`.

`.cargo/config.toml` sets, for whisper.cpp only:

- `GGML_NATIVE=OFF`: a portable AVX2 build.
- `CMAKE_GENERATOR=Ninja`.
- MSVC `/O2`, which the `cmake` crate drops (10× slower otherwise).
- `/EHsc`: whisper.cpp uses C++ exceptions.

CI installs the pinned Vulkan SDK installer after verifying its SHA-256
(`81f474711e9042f4cd22b31b2f7a8870db2e428b21586fb43dd80150be97310d`).

End users need none of this. At run time SERSHI only uses the GPU driver's
`vulkan-1.dll`, and falls back to the CPU.

Linux and macOS compile SERSHI without voice. The voice adapters are
`cfg(windows)`; `get_voice_status` reports `supported: false`.

## Measurements (reference machine)

Machine: i5-12450HX (8 cores / 12 threads, 2.4 GHz base), 32 GB RAM,
NVIDIA RTX 3050 6 GB Laptop GPU (driver 577.05) plus Intel UHD 770,
Windows 11 Home 26200. Other applications were using about 30–50 % CPU, so
these are real-world numbers.

| What | Result |
| --- | --- |
| Microphone open (WASAPI shared, 48 kHz stereo) | 51 ms |
| Microphone release | 9 ms |
| Model verify + load (Small q8_0, CPU) | ≈0.5 s |
| GPU load + warm-up, first run of a program | ≈25–28 s (one-time; then ≈1.9 s) |
| Speech synthesis of a short reply | ~150 ms |
| Idle CPU with the microphone off | unchanged vs Gate 2B |

Recognition times per profile are in the models table above.

With real speech, the time from the last word to the transcript was:

- Fast: median ≈ 1.9 s;
- Accurate: median ≈ 3.1 s;
- Prompt 3: ≈ 5 s.

Details and the abort-callback bug fixed after that run are in
[WINDOWS_PLATFORM.md](WINDOWS_PLATFORM.md#gate-3b--low-latency-voice).

## Tests

- **Portable (Ubuntu + Windows CI):**
  - the state machine and service voice API, including security
  - intents
  - resampling (anti-aliasing, DC, duration)
  - level meter
  - endpointing fixtures
  - transcript assessment
  - model catalog, model store (tamper, short, oversize, cancel, corrupt)
  - WAV decoding
  - contract tests (Gate 3A)
  - frontend voice UI
- **Windows, always:** `tests/windows_voice.rs::lists_microphones_voices_and_privacy_state`
  (read-only).
- **Windows, hardware (ignored by default):**

  ```powershell
  $env:SERSHI_STT_MODEL = "$env:LOCALAPPDATA\dev.sershi.desktop\models\stt\ggml-small-q8_0.bin"
  cargo test -p sershi-platform --test windows_voice -- --ignored --nocapture --test-threads=1
  ```

  These cover an offline TTS → STT round trip for each installed language,
  silence rejection, opening and releasing the default microphone, and
  playing and stopping speech.

## Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| "Microphone unavailable … permission" | Windows Settings › Privacy & security › Microphone: turn on *Microphone access* and *Let desktop apps access your microphone*. |
| "The microphone sent only silence" | Muted hardware switch, or the privacy switch above. |
| "No microphone was found" | Connect one; Settings › Voice lists what Windows sees. |
| Chosen microphone missing | SERSHI uses the system default and says so. |
| "Spoken replies aren't available" | No voice for that language: install the language's speech pack (Windows Settings › Time & language › Speech). |
| Slow recognition | Choose your conversation language instead of Automatic, or the Base model. |

## Wake word (future, not implemented)

Planned only after Gate 3A passes (Prompt 3B):

```text
user enables it explicitly → local detector (WakeWordPort) inside SERSHI
→ the same unmistakable Listening indicator → push-to-talk pipeline
```

- Processing is local by default.
- The phrase is user-changeable where technically feasible; "SERSHI" is the
  likely default, but no pronunciation is hard-coded.
- **Wake word ≠ authorization, voice ≠ identity, voice ≠ approval.**
- Voice will stay decoupled from future roles and skills:
  `voice → agent → role → skill → typed capability → policy`.

## Licenses

| Component | Version | License | Distribution |
| --- | --- | --- | --- |
| `cpal` | 0.18 | Apache-2.0 | Linked |
| `whisper-rs` / `whisper-rs-sys` | 0.16 / 0.15 | Unlicense | Linked |
| whisper.cpp / ggml (vendored by whisper-rs-sys) | bundled | MIT | Statically linked; include its notice with binaries |
| `sha2` | 0.10 | MIT OR Apache-2.0 | Linked |
| Whisper models (GGML conversions) | revision above | MIT (OpenAI Whisper) | Downloaded by the user; not redistributed |
| Windows speech voices | OS | Windows license | Not distributed |

All are compatible with SERSHI's Apache-2.0 license.
