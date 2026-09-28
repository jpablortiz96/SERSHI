# Product

## What SERSHI is

**SERSHI — your intelligent desktop companion.** An open-source AI agent that lives
on your Windows computer: a calm, animated presence you can talk or type to, that
understands your intent and acts on your behalf through safe, visible, permissioned
tools.

Instead of navigating apps, files, settings, terminals and dashboards, you tell
SERSHI what you want:

- "Open Spotify." · "Open my Lumaria project in VS Code."
- "How much RAM am I using?" · "Find the Excel file I downloaded yesterday."
- "What am I looking at?" · "Why is this error appearing?"
- "Start work mode." · "When I say let's code, open VS Code, GitHub and Spotify."

SERSHI grows from **assistant → operator → agent → personal orchestration layer**.

## Principles (non-negotiable)

1. **Local-first.** Meaningfully useful without paid API keys; cloud providers are
   optional adapters.
2. **Privacy-first.** No hidden recording, no silent screen capture, no analytics.
3. **Safe execution.** Models propose typed tool calls; policy decides; the user
   confirms anything risky. Never model → shell.
4. **Beautiful by default.** Design quality is an engineering requirement.
5. **Motion communicates state.** You should know what SERSHI is doing before
   reading anything.
6. **Provider-independent** and **extensible** through skills.
7. **Consumer-friendly.** One installer; no developer tools for end users.

## Surfaces

| Surface             | Purpose                                                                                           | Status        |
| ------------------- | ------------------------------------------------------------------------------------------------- | ------------- |
| Floating companion  | Always-present, state-driven core above your apps; click to summon, drag to move                  | Prototype     |
| Command Center      | Mission control: conversation, state, telemetry, activity, settings                               | Prototype     |
| Quick Command       | `Alt+Space` compact command surface: type or speak, execute, disappear                            | Designed (v0.1) |
| Voice               | Wake word, push-to-talk, spoken replies                                                           | Designed (v0.3) |
| Confirmations       | Concrete "what / what data / why / remember?" approvals                                           | Designed (v0.1) |
| Onboarding          | Choose how SERSHI thinks, listens and speaks; grant permissions                                   | Designed (v0.9) |

### Information architecture

```text
Command Center
├── Home          core · status · conversation · command bar · telemetry rail · activity rail
├── Activity      full audit log of this session
├── Settings      privacy · tools & permissions · platform capabilities · developer · about
└── (later)       Skills · Routines · Memory · Connections · Developer Mode
```

Screens appear when they have something real to show; there are no empty
placeholder sections.

### Quick Command (v0.1)

```text
Alt + Space
╭──────────────────────────────────────╮
│ ◉  open spotify                  ↵   │
╰──────────────────────────────────────╯
  Opens Spotify · safe · system.apps.launch
```

A single floating input with a preview of the tool it will use; results appear as a
companion notification. Same pipeline and policy as everything else.

### Onboarding (v0.9)

```text
Welcome to SERSHI
How SERSHI thinks      ○ Local AI (Ollama)  ○ OpenAI  ○ Anthropic  ○ Google
Speech recognition     ○ Local  ○ Cloud  ○ Disabled
Voice                  ○ Local  ○ ElevenLabs  ○ Disabled
Wake word              "SERSHI"   [ on / push-to-talk only ]
Permissions            Microphone Allow · Screen Ask · Files Selected folders
                       · System actions Safe actions only
```

## Developer Mode (planned)

Tool events, IPC diagnostics, provider diagnostics, state transitions and
performance metrics — never secrets. Today it consists of the state preview in
Settings (developer builds only).

## Out of scope before v1.0

See [ROADMAP.md](ROADMAP.md#explicitly-out-of-scope-before-v10).
