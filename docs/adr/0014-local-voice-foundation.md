# 0014 — Local voice: push-to-talk, native audio, local models, no authority

**Status:** Accepted · 2026-09

## Context

Prompt 3 gives SERSHI a voice. It must hear the user and answer out loud,
fully offline once a model is installed, on the Windows machines SERSHI
targets. It must not weaken the Gate 1A security model.

Several shortcuts were available, and each would have been a mistake:

- capturing audio in the WebView
- a Python helper
- a cloud API key
- a "voice approve" path
- an always-on microphone

## Decision

### Audio does not grant authority

- **Voice is an input and an output modality, never authorization.** A
  recognised transcript is untrusted text. It enters through
  `AssistantService::submit_transcript`, which calls the same `submit` as a
  typed command, so it follows the same path: intent → typed `ToolCall` →
  policy → confirmation.
- **No path to approval.**
  - There is no approve intent.
  - Voice code never reaches `decide`, and never learns a confirmation id.
  - Pressing the microphone while an approval is pending **cancels** the
    approval, because a new request replaces a pending one. The microphone
    and a confirmation window never overlap.
  - "Sí", "yes" or "approve" spoken aloud are ordinary text that SERSHI does
    not act on.
  - "Cancel" is recognised as an exact utterance, because cancelling only
    reduces authority.
- **Capabilities.** The nine voice commands are granted to the Command
  Center only. The companion receives a bounded 0–1 level event. The
  confirmation window gains nothing and bundles no voice code.

### Push-to-talk only; wake word deferred

- The microphone is off at start-up and opens only on an explicit click.
- It is visibly indicated in:
  - the mic button
  - the Listening state on the Core and the companion
  - the tray tooltip
  - an activity entry: "Microphone on / off", with the duration
- It closes on endpoint, a second click, Escape, hiding the Command Center,
  a typed command, or quit.
- There is a hard 30 s cap.
- `WakeWordPort` is defined, has no implementation, and nothing calls it.

### Native capture and playback

- Audio is captured and played in the Rust process with `cpal` (WASAPI
  shared mode). It shares the `windows 0.62` crate Tauri already links.
- Each capture session runs on its own thread and owns the device handle,
  so the microphone is released deterministically.
- Audio callbacks only send over channels. No lock is held while joining an
  audio thread.
- Raw audio never crosses IPC and is never written to disk or logs.

### Local recognition (STT)

`SpeechToTextPort` is implemented with `whisper-rs` (whisper.cpp), in
process.

- **Model.** The default is Whisper Small (q8_0, 264 MB); Base and Large v3
  Turbo are also offered. Models are data:
  - downloaded only when the user asks
  - from a pinned Hugging Face revision, over HTTPS (WinHTTP, TLS 1.2+, no
    downgrade redirects)
  - streamed into a `.partial` file whose size cannot exceed the expected
    size
  - checked for exact size and SHA-256, then atomically renamed
  - verified again before the first load in each session
- **Build.** whisper.cpp is built with:
  - `GGML_NATIVE=OFF`: a portable AVX2 baseline, not `-march=native`
  - explicit MSVC `/O2`: the `cmake` crate drops it otherwise, which
    measured about 10× slower
- **Why not a sidecar.** A whisper.cpp sidecar would need the audio in a
  temporary file, an external process, and a second binary to ship and
  verify.

### Local synthesis (TTS)

`SpeechSynthesisPort` is implemented with Windows'
`Windows.Media.SpeechSynthesis`: the OneCore voices installed with Windows
language packs.

- The text is plain (not SSML).
- The output is an in-memory WAV, played by SERSHI's own output adapter.
  That gives a real playback lifecycle (the Speaking state) and a real
  output envelope for the companion.
- No shell or PowerShell is involved, and no second model is shipped.

### Half-duplex conversation

`Listening → Transcribing → Thinking → … → Speaking → Ready`.

- SERSHI stops talking before it listens.
- It never speaks while the microphone is open.
- `Transcribing` is a real state: the microphone is already off. Without
  it, Listening would have been a lie, or Thinking would have rejected the
  transcript as "busy".

### Language

- The conversation language is a BCP-47 tag or automatic. It is separate
  from the interface locale.
- Replies are phrased in the interface language by the same code that writes
  the transcript, then spoken with a voice for that language.

## Alternatives

- **WebView `getUserMedia` capture.** Rejected: raw audio would live in the
  semi-trusted renderer, and there would be WebView2 permission prompts.
- **Silero/ONNX VAD.** Deferred: it needs an ONNX runtime. A conservative
  energy detector with an adaptive floor is enough for endpointing, which is
  UX, not security.
- **SAPI 5 TTS.** Rejected: it is older, and on the reference machine it
  exposes a different (desktop) voice set. WinRT gives a clean in-memory
  stream.
- **Cloud STT/TTS.** Out of scope. It will come later behind the
  Connector/Credential architecture.
- **Voice approval.** Rejected permanently: voice ≠ identity ≠ approval.

## Consequences

- **Build prerequisites.** Windows builds now need CMake and libclang (LLVM).
  CI's `windows-latest` has both. Ubuntu CI does not build the voice
  adapters: they are `cfg(windows)`, and portable logic is tested
  everywhere.
- **MSRV.** Raised to 1.88. whisper-rs-sys requires it, and the code already
  used let-chains.
- **Latency on CPU.** Recognition is seconds, not milliseconds. On the
  reference i5-12450HX it takes about 3 s with a fixed language and about 6 s
  on Automatic (whisper.cpp encodes twice to detect the language). Prompt 3B
  should evaluate clang-cl builds, GPU backends and streaming.
- **Memory.** A loaded model stays resident while voice is in use. Changing
  the model releases the previous one.
- **Wake word.** Gated on Gate 3A.
