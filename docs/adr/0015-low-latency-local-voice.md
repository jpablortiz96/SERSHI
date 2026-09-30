# 0015 — Low-latency local voice: Vulkan GPU with CPU fallback, two measured profiles

**Status:** Accepted · 2026-09

## Context

Physical testing of Prompt 3 showed about 5 s between the end of a spoken
command and SERSHI acting. Instrumenting the pipeline showed where the time
went:

- recognition: Whisper Small on the CPU took 5–8 s (3–6 s with a fixed
  language);
- a fixed 1 s end-of-speech wait.

Intent, policy and tools took milliseconds.

## Decision

- **GPU through Vulkan, CPU always.**
  - whisper.cpp is built with its Vulkan backend: cross-vendor, and it
    needs only the driver's `vulkan-1.dll` at run time.
  - `vulkan-1.dll` is delay-loaded and loaded from System32 only. Without
    it, whisper.cpp is never called and voice reports itself unavailable
    rather than crashing.
  - A failed GPU load (e.g. not enough video memory) falls back to the
    CPU.
  - Measured on an RTX 3050 Laptop: Small drops from 5.7–10.6 s to 0.4–0.9 s.
  - CUDA was not adopted: a 3 GB toolkit, NVIDIA only, and large
    redistributable DLLs. It stays possible later behind the same
    `Acceleration` choice.
- **Two profiles, both benchmarked.**
  - **Fast** = Whisper Small q8_0 (default).
  - **Accurate** = Large v3 Turbo q8_0 (GPU needed to be interactive).
  - Whisper Base (inaccurate in Spanish) and Turbo q5_0 (slower than q8_0
    on the GPU) were dropped.
- **Adaptive end of speech.** 600 ms after short commands, 900 ms after
  longer speech (was 1 000 ms). A developer-only override supports tuning.
- **Early decode.** Decoding starts after a 200 ms pause. It is used only
  if no speech followed; it is aborted otherwise.
- **Warm engines and warm-up.**
  - Engines stay loaded between commands and are released after 15 idle
    minutes.
  - GPU pipelines are compiled in the background (fixed language,
    detection, context) instead of on a user's first command.
- **Automatic keeps detecting every utterance.** Reusing the last language
  as a hint made Whisper translate English into Spanish. A chosen language
  skips detection.
- **Recognition context.** A short list of well-known installed app names
  helps spelling. It is context, never authority.
- **Security unchanged.**
  - Only the final transcript, after capture ends, is submitted, through
    the typed-command path.
  - Early, partial, warm-up and cancelled results never execute (tested).
  - The Accurate second pass only re-reads an *unclear* Fast result; it
    never lowers the bar for acting.

## Alternatives

- **faster-whisper.** Needs a Python/CTranslate2 runtime, which conflicts
  with "no Python for users", and has no measured advantage over GPU
  whisper.cpp here.
- **Shrinking Whisper's audio window.** About 0.35 s, but it broke one-word
  replies ("Sí" → "CEE") and compiled new GPU pipelines per length.
- **clang-cl builds.** No CPU gain over MSVC in measurement.
- **Session language hints.** Rejected (translation).

## Consequences

- **Build prerequisites.** Windows builds need the Vulkan SDK and Ninja,
  besides CMake and LLVM (docs/VOICE.md#building). CI installs the Vulkan
  SDK from a SHA-256-pinned installer.
- **First run on a machine.** The first GPU use of a new SERSHI binary
  compiles shaders (~25 s), in the background.
- **Memory.** A warm GPU engine uses video memory:
  - Fast: ~264 MB of model weights;
  - Accurate: ~874 MB.
  It is released after 15 idle minutes.
