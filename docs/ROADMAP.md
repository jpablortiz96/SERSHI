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
- ⏳ Manual Windows validation ([checklist](WINDOWS_PLATFORM.md#manual-validation-checklist))

## v0.1 — Operator

**Objective:** the v0.1 journey works on Windows: launch → companion → "Open
Spotify" → policy → Spotify opens → success.

- Windows app launcher (`system.open_application`), safe close, battery, open folder,
  file search in permitted folders
- Confirmation UI and one-time / remembered grants
- `sershi-storage` (SQLite): settings, grants, persistent activity
- `CredentialStore` (Windows Credential Manager)
- First `LlmProvider` (Ollama, local) + model-backed `IntentResolver`; keyword
  resolver as fallback
- Tray icon, global shortcut (Command Center + Quick Command `Alt+Space`),
  companion click-through and size/position settings
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

- Local wake word, VAD, STT (whisper.cpp / faster-whisper), TTS (Piper)
- Push-to-talk, mute, wake word off; real audio-driven visualisation
- Optional cloud STT/TTS adapters (e.g. Deepgram, ElevenLabs)

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
