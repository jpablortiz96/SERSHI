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
| `AssistantState`      | `sershi-core::assistant`               | 11 states; see [ARCHITECTURE.md](ARCHITECTURE.md#assistant-state)                        |
| `AssistantSnapshot`   | `assistant`                            | state + preview + revision; broadcast to every surface                                   |
| `CommandRequest`      | `service`                              | Raw user text (≤ 1000 chars). Never persisted by the core                                 |
| `Intent`              | `intent`                               | `UseTool(ToolCall)` · `Reply` · `NotYetAvailable` · `NotUnderstood`                       |
| `ToolDefinition`      | `tool`                                 | id, name, description, input/output JSON Schema, permissions, risk, timeout, platforms    |
| `ToolCall`            | `tool`                                 | tool id + typed JSON input + origin (`User` / `Agent` / `Routine`)                       |
| `ToolOutput`          | `tool`                                 | structured `data` + tool-composed human `summary`                                        |
| `RiskLevel`           | `tool`                                 | `safe` · `sensitive` · `highRisk`                                                        |
| `PermissionId`/`PermissionState` | `ids`, `permission`         | namespaced id; `granted` · `ask` · `denied`                                              |
| `PolicyDecision`      | `policy`                               | `Allow` · `Confirm { reason, can_remember }` · `Deny(reason)`                            |
| `ConfirmationRequest` | `executor`                             | What the user must approve                                                                |
| `ExecutionOutcome`    | `executor`                             | `completed` · `confirmationRequired` · `denied` · `failed`                               |
| `CommandOutcome`      | `service`                              | What the UI receives: status, tool id, data, structured `detail`, duration, and a canonical English `reply` used as fallback |
| `OutcomeDetail`       | `service`                              | `answer` · `unavailable` (capability id, milestone) · `denied` (reason) · `rejected` (reason) — lets surfaces phrase replies in the interface language |
| `ActivityEntry`       | `activity`                             | Audit record; never contains user content                                                |
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
3. **Plan** (v0.1+). Multi-step requests become an `AgentPlan`; the Planning state
   is shown; each step is still an individual `ToolCall`.
4. **Policy.** `PolicyEngine::evaluate` decides from the _registered_ definition,
   the grants, the platform and the origin ([ADR 0003](adr/0003-agent-tool-security-model.md)).
5. **Confirmation.** If required, the call is returned as a `ConfirmationRequest`
   and not executed (implemented). In v0.1 the confirmation UI lets the user approve,
   which re-submits the same call with a one-time grant.
6. **Execution.** The tool parses its input into a typed struct
   (`deny_unknown_fields`), calls its port, and returns data plus a summary.
7. **Audit.** The executor records requested / denied / confirmation / completed /
   failed activity with durations, without payloads.
8. **Response.** The service maps the outcome to a `CommandOutcome` and a state
   (`Success`, `Warning`, `Error`), then settles to `Idle`.

## Tools shipped today

| Tool                | Risk | Permission         | Platforms                 | Status on Windows              |
| ------------------- | ---- | ------------------ | ------------------------- | ------------------------------ |
| `system.get_info`   | safe | `system.info.read` | windows, linux, macos     | REQUIRES_WINDOWS_VALIDATION    |
| `system.get_memory` | safe | `system.info.read` | windows, linux, macos     | REQUIRES_WINDOWS_VALIDATION    |
| `system.get_cpu`    | safe | `system.info.read` | windows, linux, macos     | REQUIRES_WINDOWS_VALIDATION    |

## v0.1 tools (designed)

| Tool                         | Risk      | Permission            | Notes                                                                      |
| ---------------------------- | --------- | --------------------- | -------------------------------------------------------------------------- |
| `system.open_application`    | safe      | `system.apps.launch`  | Resolves a known app (App Paths, Start menu) to an executable; never a shell string |
| `system.close_application`   | sensitive | `system.apps.close`   | Graceful close (`WM_CLOSE`) of an app SERSHI can identify; never force-kill by default |
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
