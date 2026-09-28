# Agent model

How SERSHI turns intent into action. The central rule: **a language model can only
propose structured calls to registered tools; it can never execute anything
directly.**

```text
LLM ──✗──▶ shell                         (prohibited, impossible by construction)

User → Request → Intent/Plan → ToolCall → Policy → Permission → Risk
     → (Confirmation) → Validated tool → Result → Audit → Response
```

## Domain entities

| Entity                | Where (today)                          | Notes                                                                                   |
| --------------------- | -------------------------------------- | --------------------------------------------------------------------------------------- |
| `AssistantState`      | `sershi-core::assistant`               | 12 states (incl. `awaitingConfirmation`); see [ARCHITECTURE.md](ARCHITECTURE.md#assistant-state)                        |
| `AssistantSnapshot`   | `assistant`                            | state + preview + revision; broadcast to every surface                                   |
| `CommandRequest`      | `service`                              | Raw user text (≤ 1000 chars). Never persisted by the core                                 |
| `Intent`              | `intent`                               | `UseTool(ToolCall)` · `Reply` · `NotYetAvailable` · `NotUnderstood`                       |
| `ToolDefinition`      | `tool`                                 | id, name, description, input/output JSON Schema, permissions, risk, timeout, platforms    |
| `ToolCall`            | `tool`                                 | tool id + typed JSON input + origin (`User` / `Agent` / `Routine`)                       |
| `ToolOutput`          | `tool`                                 | structured `data` + tool-composed human `summary` + optional trusted `subject` (e.g. the resolved application name, for activity) |
| `Prepared`            | `tool`                                 | Result of `Tool::prepare`: the confirmation action and trusted `ConfirmationSubject`     |
| `RiskLevel`           | `tool`                                 | `safe` · `sensitive` · `highRisk`                                                        |
| `PermissionId`/`PermissionState` | `ids`, `permission`         | namespaced id; `granted` · `ask` · `denied`                                              |
| `PolicyDecision`      | `policy`                               | `Allow` · `Confirm { reason, can_remember }` · `Deny(reason)`                            |
| `ConfirmationRequest` | `confirmation`                         | What the confirmation window shows: CSPRNG id, tool id (display), action, trusted subject, risk, `level`, reason, expiry, `canRemember: false`. Never sent to the Command Center |
| `ConfirmationDecision`| `service`                              | The confirmation window's only input: `{ confirmationId, decision: "approve" \| "cancel" }` |
| `ConfirmationStore`   | `confirmation`                         | The single pending call, kept in Rust; one-time, expiring                                  |
| `SurfaceAssignment`   | `confirmation`                         | Which confirmation the trusted window was opened for (used by the shell)                   |
| `ExecutionOutcome`    | `executor` (internal)                  | `Completed` · `ConfirmationRequired` · `Denied` · `Declined` · `Failed` · `SubjectChanged` |
| `ApplicationDescriptor` / `ApplicationSummary` | `apps::model`  | Discovered application (internal) / its path-free projection for the UI ([APPLICATIONS.md](APPLICATIONS.md)) |
| `ApplicationResult`   | `apps::tools`                          | Structured app tool data: `opened` · `notFound` · `ambiguous` · `launchFailed` · `closeRequested` · `notRunning` · `closeUnsupported` · `catalogUnavailable` |
| `CommandOutcome`      | `service`                              | What the UI receives: status (incl. `unresolved`, `cancelled`, `expired`), tool id, data, structured `detail`, `confirmation`, duration, and a canonical English `reply` used as fallback |
| `OutcomeDetail`       | `service`                              | `answer` · `unavailable` (capability id, milestone) · `denied` (reason) · `rejected` (reason) — lets surfaces phrase replies in the interface language |
| `ActivityEntry`       | `activity`                             | Audit record; never contains user content. `subject` holds only a resolved display name from trusted data |
| `PlatformCapability`  | `platform`                             | Honest per-platform capability status                                                    |
| `AgentPlan`           | _v0.1+_                                | Ordered `ToolCall`s with dependencies; rendered as the Planning state                    |
| `Conversation`/`Message` | UI session store (`state/conversation.ts`) | Session-only today; persisted metadata in v0.7                                  |
| `MemoryEntry`, `Routine`, `Skill`, `Provider`, `UserSetting` | _planned_ | See [MEMORY.md](MEMORY.md), [SKILLS.md](SKILLS.md), [ROADMAP.md](ROADMAP.md) |

**Serialization.** All IPC entities are serde types with camelCase JSON, exported
to TypeScript by `ts-rs`. Enums are string unions; tagged unions use a `kind` field.
64-bit integers are exported as `number` (values stay below 2⁵³).

**Ownership.** The core owns state, grants, activity and tool definitions. The UI
owns only presentation and the session transcript.

## Pipeline, step by step

1. **Request.** Text arrives from the command bar (later: voice transcript, routine
   trigger, quick command). Validated for length and emptiness; rejected while busy.
2. **Intent.** An `IntentResolver` maps the request to an `Intent`.
   - Today: `KeywordIntentResolver` — deterministic, small, honest, with English,
     Spanish and Portuguese keywords ([LOCALIZATION.md](LOCALIZATION.md)). Calls it
     produces carry `CallOrigin::User` because they are a direct mapping of the
     user's words.
   - v0.1: an LLM-backed resolver presents tool definitions as function schemas and
     returns structured calls with `CallOrigin::Agent`. Free-form text output is a
     reply, never an action.
3. **Plan.** The Planning state covers policy and the tool's own `prepare`
   step (e.g. resolving "Notepad" against the application catalog and checking
   it is running). `prepare` is read-only; it can answer early ("not found",
   "ambiguous", "isn't running") without asking for approval. Multi-step
   requests become an `AgentPlan` in v0.1+; each step is still an individual
   `ToolCall`.
4. **Policy.** `PolicyEngine::evaluate` decides from the _registered_ definition,
   the grants, the platform and the origin ([ADR 0003](adr/0003-agent-tool-security-model.md)).
5. **Confirmation** (implemented). If required, the call is stored in the
   `ConfirmationStore` and the state becomes `AwaitingConfirmation`; Rust opens
   the dedicated confirmation window, which alone receives the
   `ConfirmationRequest`. Approving there runs the **stored** call once,
   re-checking policy and the resolved subject
   ([ADR 0010](adr/0010-trusted-confirmation-lifecycle.md),
   [ADR 0011](adr/0011-dedicated-confirmation-surface.md)). Cancel, closing the
   window, expiry (60 s), a new request, dismissal or quitting discard it.
   **An agent can request an action but can never approve its own request**:
   no resolver, tool or model has a path to `decide`.
6. **Execution.** The tool parses its input into a typed struct
   (`deny_unknown_fields`), calls its port, and returns data plus a summary.
7. **Audit.** The executor records requested / denied / confirmation / completed /
   failed activity with durations, without payloads.
8. **Response.** The service maps the outcome to a `CommandOutcome` and a state
   (`Success`, `Warning`, `Error`, `AwaitingConfirmation`), then settles to `Idle`.

## Tools shipped today

| Tool                | Risk | Permission         | Platforms                 | Status on Windows              |
| ------------------- | ---- | ------------------ | ------------------------- | ------------------------------ |
| `system.get_info`   | safe | `system.info.read` | windows, linux, macos     | REQUIRES_WINDOWS_VALIDATION    |
| `system.get_memory` | safe | `system.info.read` | windows, linux, macos     | REQUIRES_WINDOWS_VALIDATION    |
| `system.get_cpu`    | safe | `system.info.read` | windows, linux, macos     | REQUIRES_WINDOWS_VALIDATION    |
| `system.open_application`  | safe      | `system.apps.launch` (granted by default) | windows | REQUIRES_WINDOWS_VALIDATION |
| `system.close_application` | sensitive | `system.apps.close` (ask)                 | windows | REQUIRES_WINDOWS_VALIDATION |

Both application tools take exactly `{ "application": "<name>" }` — never a
path, command line or arguments — and resolve it against the discovered
catalog. Close always confirms and only sends `WM_CLOSE`. See
[APPLICATIONS.md](APPLICATIONS.md).

## v0.1 tools (designed)

| Tool                         | Risk      | Permission            | Notes                                                                      |
| ---------------------------- | --------- | --------------------- | -------------------------------------------------------------------------- |
| `system.get_battery`         | safe      | `system.info.read`    | `GetSystemPowerStatus`                                                     |
| `files.open_folder`          | safe      | `files.folders.open`  | Known folders + user-approved roots                                        |
| `files.search`               | safe      | `files.read`          | Only within permitted directories                                          |

Each ships with policy tests, a capability entry marked
`requiresWindowsValidation`, and a manual validation step.

## Response composition and personality

SERSHI's default voice is calm, concise, warm and capable. Tool summaries are
factual ("You're using 12.0 GB of 32.0 GB memory (38%)."). A future response
composer may rephrase them in the configured personality, but:

- personality is configuration for wording only;
- it can never alter permissions, risk, confirmation or policy;
- facts shown to the user come from tool output, not from the model's memory.

## Error handling

User-facing errors explain what happened and what to do, never raw codes:

> I couldn't complete Memory usage. The system didn't return the information — try
> again in a moment.

Raw details are reserved for Developer Mode diagnostics (with redaction).
