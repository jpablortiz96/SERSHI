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
- The utterance ends after 1.0 s of silence following speech.
- With no speech after 8 s, the result is "No speech detected".
- The hard cap is 30 s; SERSHI then transcribes what it heard.
- A stream of exact zeros is reported as a silent microphone. Windows
  privacy settings do this to desktop apps.

The detector is tested with generated audio (tones, syllable-modulated
"speech", deterministic noise); no recordings are committed. Thresholds are
tuned in Gate 3A.

## Speech recognition

The engine is **whisper.cpp** through `whisper-rs` 0.16 (bindings
Unlicense, whisper.cpp MIT), in process, CPU only:

- greedy decoding
- single segment
- no timestamps
- non-speech tokens suppressed
- 8 threads at most

Language:

- **Automatic**: Whisper detects the language.
- **English / Español / Português**: the language is a hint. This skips
  detection and is about twice as fast (whisper.cpp runs the encoder twice
  to detect).

The interface language never changes because of what you said.

### Models

Models come from `huggingface.co/ggerganov/whisper.cpp`, revision
`5359861c739e955e79d9a303bcbc70fb988958b1`. They are OpenAI Whisper
weights (MIT).

| Id | File | Download | Memory (approx.) | Trade-off |
| --- | --- | --- | --- | --- |
| `whisper-base-q8` | `ggml-base-q8_0.bin` | 82 MB | +91 MB | Fastest. Unreliable outside English in testing: a Spanish clip was heard as "Have a nice party, Faye", and once as Greek with 0.89 confidence |
| `whisper-small-q8` (default) | `ggml-small-q8_0.bin` | 264 MB | +274 MB | Balanced. Good EN/ES/PT |
| `whisper-large-v3-turbo-q5` | `ggml-large-v3-turbo-q5_0.bin` | 574 MB | +583 MB | Most accurate. About 34 s per utterance on the reference CPU with a language hint |

Memory is the measured increase in resident memory after loading, on the
reference machine; recognition adds about 15 MB while it runs.

SHA-256 values are in `crates/sershi-core/src/voice/models.rs`.

**Why Small q8_0.** q8_0 was faster than both q5_1 and f16 on the reference
CPU (9.3 s vs 11.4 s and 11.5 s on Automatic) at equal or better accuracy.
Base mis-hears Spanish; Large v3 Turbo is too slow on CPU.

**Download security:**

1. The user clicks Download (the size is shown first).
2. `https://` with a pinned URL.
3. The body is streamed to `<file>.partial`, never beyond the expected size.
4. Exact size and SHA-256 must match; otherwise the file is deleted.
5. `fsync`, then an atomic rename.
6. Before the first load in each session, the file is hashed again (0.4 s).
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
prerequisites:

- **CMake**. Visual Studio Build Tools ships one; add
  `…\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin` to
  `PATH`, or install CMake.
- **libclang** for `bindgen`: install LLVM (`winget install LLVM.LLVM`), or
  set `LIBCLANG_PATH` to a folder containing `libclang.dll`.
  whisper-rs-sys's bundled bindings are generated on Linux and do not match
  MSVC's type sizes.
- Also keep the path to the repository short: MSBuild's file tracker fails
  on very long paths.

`.cargo/config.toml` sets `GGML_NATIVE=OFF` (portable AVX2 build) and
MSVC `/O2` for whisper.cpp; see the comments there. The first build compiles
whisper.cpp in about a minute.

Linux and macOS compile SERSHI without voice. The voice adapters are
`cfg(windows)`; `get_voice_status` reports `supported: false`.

## Measurements (reference machine)

Machine: i5-12450HX (8 cores / 12 threads, 2.4 GHz base), 32 GB RAM,
Windows 11 Home 26200. Other applications were using about 35 % CPU during
the tests, so these are real-world numbers, not a clean bench.

| What | Result |
| --- | --- |
| Microphone open (WASAPI shared, 48 kHz stereo) | 51 ms |
| Microphone release | 9 ms |
| SHA-256 verify of the default model | see Gate 3A record |
| Model load (Small q8_0) | 275 ms (after verify) |
| Recognition, ~1.5 s utterance, language set | ~3.0 s |
| Recognition, same, Automatic | ~5.9 s |
| Speech synthesis of a short reply | ~150 ms |
| Idle CPU with the microphone off | see Gate 3A record |

Recognition dominates latency. Prompt 3B should evaluate clang-cl builds of
ggml (MSVC's ggml is known to be slower), GPU backends (Vulkan) and
detecting the language on a shortened window.

Recognition accuracy on synthetic speech: Windows' Spain-Spanish voice
saying "Abre Spotify" is heard as "Hables Spotify" by Small. Real speech
(Gate 3A) is the reference. The resolver accepts the second-person forms
"abres/abras/cierras", which are real request phrasings.

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
