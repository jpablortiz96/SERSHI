# Architecture

SERSHI is a Windows-first desktop application built on **Tauri 2**: a Rust process
owns everything privileged, and React renders two windows that only ever _ask_ the
Rust side for things through narrow, typed IPC commands.

This document describes the system as it exists now and the boundaries that later
versions must respect. Decisions and their alternatives are recorded in
[`adr/`](adr/).

## System overview

```mermaid
flowchart TB
    subgraph UI["Experience layer — WebView (React, TypeScript)"]
        CC["Command Center<br/>window: main"]
        CO["Floating companion<br/>window: companion"]
        IPCC["src/ipc — typed client<br/>(only module allowed to import Tauri)"]
        CC --> IPCC
        CO --> IPCC
    end

    subgraph Shell["Desktop shell — apps/desktop/src-tauri (Rust)"]
        CMD["IPC commands<br/>per-window permissions"]
        EV["Event broadcaster"]
        WIN["Window behaviour"]
    end

    subgraph Core["sershi-core — portable domain (Rust)"]
        SVC["AssistantService<br/>(application layer)"]
        SM["State machine"]
        INT["IntentResolver"]
        EXE["ToolExecutor"]
        POL["PolicyEngine"]
        REG["ToolRegistry"]
        ACT["ActivityLog"]
        PORTS["Ports<br/>(SystemInfoProvider, …)"]
    end

    subgraph Platform["sershi-platform — adapters (Rust)"]
        SYS["SysinfoSystemInfo<br/>(portable)"]
        CAP["Capability report"]
        WINMOD["windows/ — cfg(windows) only"]
    end

    IPCC -- "invoke (typed)" --> CMD
    EV -- "events" --> IPCC
    CMD --> SVC
    SVC --> SM & INT & EXE & ACT
    EXE --> POL & REG
    REG -. "tools depend on" .-> PORTS
    SYS -- implements --> PORTS
    CMD --> CAP
```

## Layers and dependency rule

Dependencies point inward. The domain knows nothing about Tauri, React, Windows or
any AI vendor.

```text
React UI  ──IPC──▶  Desktop shell  ──▶  Application (AssistantService)
                                             │
                                             ▼
                                   Domain (state, tools, policy, activity)
                                             │
                                             ▼
                                        Ports (traits)
                                             ▲
                                             │ implements
                                   Adapters (sershi-platform, future providers)
```

| Unit                          | Kind          | Owns                                                                                                                  | May depend on                               |
| ----------------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| `crates/sershi-core`          | Rust lib      | Assistant state machine, tool model, policy, permissions, activity, intent boundary, application service, IPC payloads | `serde`, `thiserror` (+ `ts-rs` behind a feature) |
| `crates/sershi-platform`      | Rust lib      | OS adapters for core ports; capability report; `windows/` module                                                      | `sershi-core`, `sysinfo`                    |
| `apps/desktop/src-tauri`      | Rust bin/lib  | Windows, IPC commands, event broadcast, timers                                                                        | `sershi-core`, `sershi-platform`, `tauri`   |
| `packages/contracts`          | TS package    | IPC types (generated from Rust), command/event map, runtime guards                                                    | —                                           |
| `packages/design-tokens`      | TS package    | Design tokens and runtime theming                                                                                      | —                                           |
| `apps/desktop/src`            | React app     | Both window UIs, stores mirroring core state                                                                          | `@sershi/contracts`, `@sershi/design-tokens`, `@tauri-apps/api` (in `src/ipc` only) |

Why so few crates: see [ADR 0002](adr/0002-process-and-module-boundaries.md). A new
crate is created when a boundary needs enforcing by the compiler (e.g. a future
`sershi-storage` so the core cannot touch SQLite), not to look organised.

## The request lifecycle

The vertical slice implemented today. Every future capability (LLM tool calls,
voice, routines) enters at the same point and passes through the same steps.

```mermaid
sequenceDiagram
    participant UI as Command Center
    participant Shell as Tauri command
    participant Svc as AssistantService
    participant Int as IntentResolver
    participant Exe as ToolExecutor
    participant Pol as PolicyEngine
    participant Tool as system.get_memory
    participant Port as SystemInfoProvider

    UI->>Shell: submit_command({ request: { text } })
    Shell->>Svc: submit(request)
    Svc->>Svc: validate (non-empty, ≤ 1000 chars, not busy)
    Svc-->>UI: event: state = thinking
    Svc->>Int: resolve(text)
    Int-->>Svc: Intent::UseTool(ToolCall)
    Svc-->>UI: event: state = executing
    Svc->>Exe: execute(call, grants)
    Exe->>Pol: evaluate(definition, call, grants)
    Pol-->>Exe: Allow
    Exe->>Tool: execute(input)
    Tool->>Port: snapshot()
    Port-->>Tool: SystemSnapshot
    Tool-->>Exe: ToolOutput { data, summary }
    Exe-->>UI: events: activity (requested, completed)
    Svc-->>UI: event: state = success
    Shell-->>UI: CommandOutcome
    Note over Shell: after 2.4 s, Settle → idle<br/>(only if revision unchanged)
```

## IPC

IPC is a trust boundary (see [SECURITY.md](SECURITY.md)). Rules:

1. **Narrow, typed commands only.** There is no `run_command`, `exec` or `invoke_tool`.
   A contract test fails if a command name even resembles generic execution.
2. **Per-window permissions.** `build.rs` declares every command; Tauri generates a
   permission per command; `capabilities/*.json` grants them per window. The
   companion can call exactly `get_assistant_snapshot` and `summon_command_center`.
3. **Types come from Rust.** `ts-rs` generates `packages/contracts/src/generated`
   from the Rust types. `pnpm contracts:generate` regenerates; CI fails on drift.
4. **Rust-serialized fixtures** (`packages/contracts/fixtures`) are validated by the
   TypeScript guards, proving both sides agree on the wire format.
5. **Responses and events are guarded at runtime** in `src/ipc`; malformed payloads
   are dropped at the edge.

| Command                   | Windows            | Purpose                                             |
| ------------------------- | ------------------ | --------------------------------------------------- |
| `get_assistant_snapshot`  | main, companion    | Current assistant state                             |
| `summon_command_center`   | companion          | Show the Command Center, `Activate` the assistant   |
| `get_system_snapshot`     | main               | Read-only telemetry (polled while visible)          |
| `get_runtime_info`        | main               | Version, platform, capabilities, tool definitions   |
| `list_activity`           | main               | Recent activity (≤ 100)                             |
| `submit_command`          | main               | Run a user command through the pipeline             |
| `dismiss_assistant`       | main               | `Awake → Idle`                                      |
| `preview_assistant_state` | main               | Developer builds only: visual preview               |
| `quit_app`                | main               | Exit                                                |

| Event                      | Payload             |
| -------------------------- | ------------------- |
| `sershi://assistant-state` | `AssistantSnapshot` |
| `sershi://activity`        | `ActivityEntry`     |

**Telemetry vs tools.** `get_system_snapshot` is the user looking at their own
machine; it bypasses the tool pipeline and is not recorded as activity (it is
polled). Anything the _assistant_ does goes through tools. Future telemetry that
reveals user data (process lists, window titles) must become a tool.

## Assistant state

One state machine in Rust (`sershi-core::assistant`) is the single source of truth.
Both windows mirror the latest `AssistantSnapshot`; the UI never computes state.

```mermaid
stateDiagram-v2
    [*] --> Idle
    Sleeping --> Idle: Wake
    Sleeping --> Awake: Activate
    Idle --> Awake: Activate
    Idle --> Sleeping: Sleep
    Idle --> Listening: StartListening
    Awake --> Listening: StartListening
    Idle --> Thinking: RequestReceived
    Awake --> Thinking: RequestReceived
    Listening --> Thinking: RequestReceived
    Thinking --> Planning: PlanStarted
    Thinking --> Executing: ExecutionStarted
    Planning --> Executing: ExecutionStarted
    Executing --> Speaking: SpeechStarted
    Thinking --> Success: Completed
    Executing --> Success: Completed
    Speaking --> Success: Completed
    Executing --> Warning: AttentionNeeded
    Executing --> Error: Failed
    Success --> Idle: Settle
    Warning --> Idle: Settle
    Error --> Idle: Settle
    Awake --> Idle: Dismiss
```

(Abridged; `transition()` in `assistant.rs` is the authoritative, exhaustively
tested definition. Any non-sleeping state can `Fail`; busy states cannot `Sleep`.)

- **States vs conditions.** _Offline_ and _private_ are conditions layered on top of
  a state, not states — they describe the environment, not what SERSHI is doing.
- **Revisions.** Every change increments `revision`. Deferred work (the settle timer)
  applies only if the revision is unchanged, so a new request is never clobbered.
- **Preview.** Developer Mode can set `previewState`; surfaces render it, but policy
  and execution only ever read `state`.

## Frontend architecture

- **Two entry points** (`index.html`, `companion.html`), one Vite build.
- **`src/ipc`** — the only module that imports `@tauri-apps/api` (ESLint-enforced).
- **`src/state`** — small Zustand stores that _mirror_ core state (assistant, activity)
  or hold session-only UI state (conversation). See [ADR 0007](adr/0007-frontend-state-management.md).
- **`src/components`** — presentational components; `Core` is a pure renderer of
  `AssistantState`, replaceable by future companion packs.
- **Styling** — CSS Modules + design tokens as CSS custom properties. No utility
  framework; no hard-coded visual values ([DESIGN_SYSTEM.md](DESIGN_SYSTEM.md)).
- **Localization** — `src/i18n` owns every user-facing string (en-US, es-419,
  pt-BR). The core returns structured outcomes; the UI phrases them in the
  interface language. See [LOCALIZATION.md](LOCALIZATION.md).
- **Browser preview** — `pnpm dev:web` runs the UI without the Rust core. Every
  surface shows honest "not connected" states; `?preview=<state>` previews visuals.

## Persistence, credentials, providers

Designed, not yet implemented — see ADRs [0004](adr/0004-persistence.md) and
[0005](adr/0005-ai-provider-abstraction.md). Summary:

- **SQLite** (via `rusqlite`, bundled) owned by a future `sershi-storage` crate for
  settings, grants, activity history, routines and memory metadata. Never secrets.
- **Secrets** in the OS credential store (Windows Credential Manager) behind a
  `CredentialStore` port.
- **AI providers** behind capability-specific ports (`LlmProvider`,
  `SpeechToTextProvider`, …). A model-backed `IntentResolver` is just another
  implementation whose tool calls carry `CallOrigin::Agent`.

## Observability

- **Activity** (implemented) is the user-facing audit log: fixed-template summaries,
  tool ids and durations; never user text, payloads, paths or secrets.
- **Diagnostics** (planned, Developer Mode): structured `tracing` events —
  `assistant.state_changed`, `tool.requested|approved|denied|started|completed|failed`,
  `permission.requested|denied`, `provider.error` — with the same redaction rules.
- **Performance** (planned): startup time, idle CPU/RAM, frame health, tool and
  provider latency. Tool latency is already recorded (`durationMs`).

## Performance principles

- Nothing heavy at startup: no models, no network, one `sysinfo` handle.
- Telemetry polls only while the Command Center is visible (2 s interval).
- The companion animates only `transform`/`opacity`; loops pause when sleeping.
- No continuous canvas or WebGL rendering.

## Adding a capability (checklist)

1. Define or reuse a **port** in `sershi-core::ports` if the capability touches the OS
   or a vendor.
2. Implement the **adapter** in `sershi-platform` (Windows-only code under `windows/`).
3. Add a **tool** with an honest `RiskLevel`, required permissions, typed input with
   `deny_unknown_fields`, and a human summary.
4. Add **policy tests** for its risk and permission behaviour.
5. Update the **capability report** (`requiresWindowsValidation` until verified).
6. Run `pnpm contracts:generate` if IPC payloads changed.
