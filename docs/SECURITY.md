# Security model

SERSHI will eventually see the user's screen, hear their microphone, read their
files and act on their computer. Security is therefore an architectural property,
not a feature. This document is the threat model and the rules that follow from
it. To report a vulnerability, see the root [`SECURITY.md`](../SECURITY.md).

## Assets

| Asset                                    | Why it matters                                |
| ---------------------------------------- | --------------------------------------------- |
| The user's files, apps and settings      | SERSHI can act on them                        |
| Credentials (API keys, OAuth tokens)     | Access to paid services and personal accounts |
| Microphone and screen content            | Highly sensitive, often incidental            |
| Conversation, memory and activity        | A detailed profile of the user                |
| Integrity of SERSHI's own decisions      | Everything above depends on it                |

## Trust boundaries

```mermaid
flowchart LR
    U(("User"))
    subgraph Untrusted
        M["Model output"]
        X["External content<br/>web · e-mail · documents · screen"]
        S3["Third-party skills (v0.5)"]
    end
    subgraph Semi["Semi-trusted"]
        W["Command Center + companion<br/>WebViews"]
    end
    subgraph Min["Minimal-authority approval surface"]
        CW["Confirmation WebView<br/>2 commands · no external content"]
    end
    subgraph Trusted["Authoritative (Rust)"]
        P["Policy engine + tool registry"]
        CS["ConfirmationStore<br/>(stored ToolCall)"]
        T["Typed tools"]
        OS["Operating system"]
    end
    U --> W
    U -- "explicit click / key" --> CW
    W -- "narrow typed IPC<br/>(can request, cannot approve)" --> P
    CW -- "{ confirmationId, decision }" --> CS
    P --> CS
    CS -- "stored call, once" --> T
    X -. "data only" .-> M
    M -- "structured tool calls" --> P
    S3 -- "declared tools" --> P
    P --> T --> OS
```

| Boundary                 | Rule                                                                                                                                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Model → system**       | Model output is untrusted data. It can only _name_ a registered tool and supply typed input. It never becomes a shell command. Risk comes from the tool definition, not the model. |
| **External content → model** | Web pages, e-mails, documents and screen text are data. Instructions inside them never override system policy, permissions or confirmation. Content is delimited as untrusted when given to a model, and actions derived from it carry `CallOrigin::Agent`, which requires confirmation for anything sensitive. |
| **WebView → Rust (IPC)** | The UI is treated as potentially compromised (e.g. by a rendering bug or injected content). It gets narrow commands only, scoped per window; no generic execution; inputs validated in Rust. The Command Center and companion can **request** actions but hold **no approval channel**. |
| **Confirmation surface → Rust** | A separate, Rust-created window whose only authority is to read its assigned confirmation and return the human decision (`{ confirmationId, decision }`). It renders only structured, SERSHI-owned fields. It is a smaller target, **not** an enclave (see below). |
| **Rust core**            | Authoritative: owns the pending `ToolCall`, policy, permissions, expiry and replay prevention. Never reconstructs an action from UI input. |
| **Skills → system**      | Third-party skills get no ambient authority: declared permissions, user approval, their own namespace, revocable. See [SKILLS.md](SKILLS.md). |

## Implemented controls (v0.0.x)

| Control                                    | Where                                                        | Verified by                                          |
| ------------------------------------------ | ------------------------------------------------------------ | ---------------------------------------------------- |
| No shell/exec tool or IPC command           | `sershi-core::tool`, `build.rs`                              | `commands.test.ts` ("no command resembles generic execution") |
| Per-window command permissions              | `src-tauri/capabilities/*.json`                              | `commands.test.ts` (companion allow-list)            |
| Policy engine on every tool call            | `sershi-core::policy`, `executor`                            | `policy.rs`, `executor.rs` tests                     |
| Denied / unconfirmed tools never execute    | `executor.rs`                                                | `confirmation_and_denial_never_execute_the_tool`     |
| High-risk actions always confirm, never rememberable | `policy.rs`                                         | `high_risk_always_confirms_and_cannot_be_remembered` |
| Model-proposed sensitive actions confirm    | `policy.rs`                                                  | `model_proposed_sensitive_actions_need_confirmation_even_when_granted` |
| Validated identifiers (no injection via ids) | `sershi-core::ids`                                          | `rejects_malformed_or_hostile_ids`                   |
| Strict tool inputs (`deny_unknown_fields`)  | `tool::parse_input`                                          | `rejects_unexpected_input`                           |
| Command length limit (1000 chars)           | `service/`                                                   | `invalid_requests_are_rejected_without_state_changes` |
| Activity never stores user text or error details | `service/`, `executor.rs`                               | `user_command_text_never_reaches_the_activity_log`, `failures_are_reported_without_leaking_details_into_activity` |
| Application tools take a name, never a path, command or argument | `apps/tools.rs` (`requested_name`, ≤ 80 chars, `\ / : < > \| " * ?` and control chars rejected) | `never_accepts_paths_commands_or_extra_fields`, `application_names_are_passed_as_data_not_commands` |
| Launch without a shell (`CreateProcessW`, `IApplicationActivationManager`; `ShellExecuteExW` only for fixed `ms-settings:` and installer shortcuts) | `sershi-platform/src/windows/launch.rs`, `packaged.rs` | Review; `cargo clippy --target x86_64-pc-windows-msvc` |
| Ambiguous or unknown names never launch     | `apps/catalog.rs`, `apps/tools.rs`                           | `ambiguous_requests_are_never_guessed`, `not_found_and_ambiguous_never_launch` |
| Close is graceful only (`WM_CLOSE`; no `TerminateProcess`, no `taskkill`) and never targets Explorer, Settings or SERSHI | `windows/close.rs`, `windows/known.rs` | Review |
| No elevation: SERSHI never requests administrator rights or triggers UAC itself | `windows/launch.rs` (`ERROR_ELEVATION_REQUIRED` → structured error) | `launch_errors_are_structured_without_internal_details` |
| Trusted confirmations (see below)           | `confirmation.rs`, `service/`, `executor.rs`                 | `service/tests.rs` confirmation suite, `confirmation.rs` tests |
| Only the confirmation window can decide confirmations; Command Center and companion cannot | `capabilities/confirmation.json` (exactly 2 commands), `command-center.json`, `companion.json`; label check in `src-tauri/src/confirmation.rs` | `commands.test.ts` ("only the confirmation window can decide confirmations", exact allow-list) |
| Decisions carry only `{ confirmationId, decision }` | `service/types.rs` (`deny_unknown_fields`)               | `decisions_carry_only_an_id_and_a_choice`            |
| The executed action is the stored call      | `service/mod.rs` `decide`                                    | `the_stored_call_is_what_runs`, `a_stale_surface_cannot_approve_a_replacement_confirmation` |
| Confirmation ids: 128 bits from the OS CSPRNG | `confirmation.rs` (`getrandom`)                            | `confirmation_ids_are_128_bit_random_values`         |
| Accidental-input guard on Approve           | `surfaces/confirmation/ConfirmationSurface.tsx`              | `confirmation-surface.test.tsx`                      |
| No paths in outcomes, activity or UI (`ApplicationSummary` only) | `apps/model.rs`, `apps/tools.rs`               | `commands.test.ts` ("no paths in outcomes"), `launch_errors_are_structured_without_internal_details` |
| Global shortcut is invocation only: `set_global_shortcut` granted to the Command Center alone; Rust validates the accelerator (≤ 64 chars at IPC, ≤ 40 parsed, ≥ 2 modifiers, no Win); registers before releasing the old one; the shortcut only summons | `sershi-core::shortcut`, `integration.rs`, `commands.rs`, `capabilities/command-center.json` | `shortcut.rs` tests; `commands.test.ts` ("personalization is not privilege") |
| No personalization commands or permissions: theme, sound, companion appearance live in presentation preferences; no command is named after them | `i18n/preferences.ts`, capabilities | `commands.test.ts` ("personalization is not privilege") |
| Tray labels validated (≤ 48 chars, no control chars); actions fixed | `ipc.rs` `TrayLabels::is_valid`             | `ipc.rs` tests                                       |
| Strict Content-Security-Policy              | `tauri.conf.json` (`script-src 'self'`, `style-src 'self'`, no remote origins) | Manual review                                |
| Frozen JS prototypes                        | `tauri.conf.json` `freezePrototype`                          | Manual review                                        |
| Only `src/ipc` may import Tauri APIs        | `eslint.config.js`                                           | `pnpm lint`                                          |
| No `unsafe` Rust outside the Win32 FFI module; no `unwrap`/`expect`/`panic` in production code | Workspace lints (`unsafe_code = "deny"`; only `sershi-platform/src/windows` opts out, each block commented) | `cargo clippy -D warnings` (Linux and Windows targets) |
| No secrets in the repository                | `.gitignore` (`.env*`, keys)                                 | Review                                               |
| No telemetry or network calls               | Fonts bundled locally; CSP `connect-src` is IPC only         | Review                                               |
| Install scripts restricted                  | `pnpm-workspace.yaml` `onlyBuiltDependencies`                | Review                                               |

## Permissions and risk

| Risk          | Examples                                                   | Default behaviour                                                                                      |
| ------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| **Safe**      | Read CPU/RAM, open an app, pause media, search allowed folders | Runs if its permissions are granted; asks if undecided                                             |
| **Sensitive** | Move/rename files, send e-mail, change settings, run a script | Runs for user-originated calls with granted permissions; **model-proposed calls always confirm**   |
| **High risk** | Delete files, payments, credential or security changes, installing software | **Always confirms; "remember" is never offered**                                  |
| **Prohibited**| Anything on the policy deny-list                            | Never runs                                                                                             |

Permissions are namespaced (`system.info.read`, `files.write`, `spotify.control`)
with per-user state `granted` / `ask` / `denied`. Granted by default:
`system.info.read` and `system.apps.launch` (opening an application the user
names is equivalent to clicking it in the Start Menu). `system.apps.close` is
`ask`, so closing always confirms. Grants are user data (persisted in v0.1), never inferred by a model, and
changing personality or prompts cannot change them.

### Trusted confirmations (implemented)

Sensitive actions (today: `system.close_application`) require explicit human
approval on a dedicated surface. Decisions: [ADR 0010](adr/0010-trusted-confirmation-lifecycle.md),
[ADR 0011](adr/0011-dedicated-confirmation-surface.md).

```text
Request (Command Center / future agent)
   → intent → tool → policy: RequireConfirmation
   → Rust stores PendingConfirmation { request, exact ToolCall }
   → Rust opens the confirmation window (label "confirmation")
   → window reads its assigned request, the human chooses Cancel / Approve
   → Rust: id matches the assigned, pending, unexpired confirmation → take() once
   → Rust re-runs policy, re-resolves the target, runs the STORED call
   → Rust destroys the window; the outcome goes to the Command Center
```

**Capability allocation**

| Surface               | Can                                                             | Cannot                                              |
| --------------------- | --------------------------------------------------------------- | --------------------------------------------------- |
| Command Center (main) | Submit requests, see that approval is pending, cancel (Escape, hide, new request) | Read confirmation ids or details, approve (`decide_confirmation`, `get_confirmation_context` not granted) |
| Companion             | Read state, summon                                              | Anything confirmation-related                       |
| Confirmation window   | `get_confirmation_context` (its assigned request only), `decide_confirmation` | Events, window control, telemetry, activity, catalog, settings, submitting commands, any other command |
| Rust core             | Owns the pending action, tool call, policy, expiry, replay prevention | —                                                   |

**Guarantees**

- **Stored, authoritative action.** The pending `ToolCall` never leaves Rust.
  The surface sends only `{ confirmationId, decision: "approve" | "cancel" }`;
  any extra field (tool, input, target, risk, permission) is rejected at
  deserialization. The frontend cannot change what runs.
- **Ids.** 16 bytes from the OS CSPRNG (`getrandom`: `ProcessPrng` on
  Windows), encoded as 32 lowercase hex characters; malformed ids are
  rejected at the boundary. Ids are **not passwords** and security does not
  depend on their secrecy: they are unpredictable, single-use, short-lived and
  bound to one stored action, and only the confirmation window — only for its
  assigned confirmation — may present one.
- **Single use.** `take()` consumes the confirmation on approve, cancel and
  expiry. Unknown or stale ids are rejected without disturbing the current
  confirmation (a stale window cannot approve or cancel a replacement).
- **One at a time.** One confirmation is pending at once; a new request
  cancels the previous one (nothing runs).
- **Expiry.** 60 s, enforced at decision time; a timer also expires it so the
  window closes and the Command Center returns to a safe state. Activity
  records the expiry.
- **Re-checks on approval.** Policy is re-run (a revoked permission still
  wins) and the application re-resolved; if it no longer matches what the
  user approved, nothing runs.
- **Closing is cancelling.** × on the surface, the OS close request (Alt+F4),
  Escape, hiding the Command Center, dismissing, and quitting SERSHI all
  cancel. Window close is never treated as approval.
- **Restart cancels.** Pending confirmations exist only in memory — never in
  `localStorage`, SQLite, files or logs. A crash or restart discards them;
  SERSHI never restores a sensitive pending action.
- **Trusted content only.** The surface shows SERSHI-owned localized copy
  selected by enums (`ConfirmationAction`, `ConfirmationLevel`, reason) plus
  the resolved subject from discovery (`ApplicationSummary`) — never request
  text, tool output, model output, web, e-mail or document content, HTML or
  Markdown.
- **Accidental-input resistance.** Cancel has initial focus (Enter cancels);
  Escape cancels. Approve is inert for 600 ms after the window is focused, is
  disarmed when the window loses focus, and accepts only a pointer press that
  began on it after arming or a fresh, non-repeated key press — so a click or
  held key that began before the window appeared cannot approve.
- **Window hardening.** Created only by Rust (the frontend has no
  window-creation permission); navigation restricted to the bundled
  `confirmation.html`; devtools disabled in release builds (the `devtools`
  feature is not enabled, and the builder also turns them off); always on
  top; the same strict CSP as every window.
- **Audit.** `confirmationRequired`, `confirmationApproved`,
  `confirmationCancelled`, `confirmationExpired` are recorded with the tool
  id and the resolved display name only — never the id, the raw input,
  paths or external content.
- **Remembering** is never offered today (`canRemember: false`).

**What the separate window is — and is not.** It shares the process, the
origin and the WebView2 runtime with the other windows, so it is **not a
security enclave**; a renderer or process compromise that escapes the
WebView sandbox is out of scope. Its value comes from a much smaller
capability set, no external or model-generated content, no general
application UI, a strict CSP and a single purpose. Tauri enforces the
per-window command allow-lists in Rust, and the two confirmation commands
also check the caller's window label.

### Defence in depth

No single control is treated as sufficient. An action runs only if all hold:
typed tool → policy engine → permission model → risk classification →
(when required) trusted confirmation surface → stored authoritative
`ToolCall` → one-time confirmation → expiry → per-window IPC capabilities.

### Invariants

- **The agent may request authority; it may never grant itself authority.**
  Approval enters only through `decide_confirmation`, granted to the
  confirmation window alone. Intent resolution, tools and (future) models have
  no path to it; typing "yes" in the Command Center is a new request, which
  cancels the pending confirmation (`typing_or_saying_yes_never_approves`).
- **External content never approves.** Web pages, e-mails, documents, screen
  OCR and model output are data; only direct human interaction with the
  confirmation surface approves.
- **Audio does not grant authority** (Prompt 3, [ADR 0014](adr/0014-local-voice-foundation.md)).
  Speech can request the same actions as text; it can never approve
  privileged execution.
  - A transcript is untrusted text submitted through the typed-command path
    (`submit_transcript` → `submit`).
  - There is no approve intent, and voice code has no path to `decide` or to
    a confirmation id.
  - Opening the microphone cancels a pending approval, so the two never
    overlap.
  - "Cancel" (an exact utterance) only reduces authority.
  - Tests: `speaking_yes_never_approves_a_sensitive_action`,
    `approval_words_are_never_an_intent` and the contract tests "Gate 3A".
  - Wake word ≠ authorization; voice ≠ identity; voice ≠ approval.

### Personalization is not privilege

Decision: [ADR 0013](adr/0013-personalization-is-not-privilege.md).
Changing the theme, sounds, companion appearance or shortcut changes how
SERSHI feels; it never changes what SERSHI is authorized to do.

- **Presentation, not policy.** Theme, motion, companion appearance and
  size, sounds, volume and the chosen shortcut are presentation preferences
  in the WebView (`sershi.preferences.v1`). Tool policy, risk levels,
  permissions and the confirmation lifecycle never read them.
- **The shortcut summons and nothing more.** It shows the Command Center and
  focuses input, exactly like the tray or the companion. With a confirmation
  pending, it brings SERSHI forward; it never approves. Rust validates every
  accelerator, and only the Command Center may change it.
- **The confirmation window is unchanged.** It follows the theme through CSS
  variables and the stored preference (no event listening, no new
  permission, still exactly `get_confirmation_context` and
  `decide_confirmation`). It imports no audio code; security never depends
  on sound.
- **Packs are data, never code.** Future companion, theme and sound packs
  may contain:
  - images, sprites, vector assets
  - animation metadata
  - token overrides
  - procedural sound parameters or audio assets
  - a manifest
  Packs are parsed as untrusted input (schema-validated, size-limited) and
  rendered by SERSHI's own renderers. There is never "download → execute
  JavaScript", no CSS injection and no IPC access. Packs are **less
  privileged than Skills**: a Skill can request permissions through the
  policy engine, while a pack cannot request anything.
- **CI enforces it.** Contract tests fail if any window other than the
  Command Center gains `set_global_shortcut`, if a theme, sound, appearance
  or pack command appears, or if the confirmation boundary changes.

### Future hardening (not implemented)

- A **critical** confirmation level for payments, credential access, security
  settings and permanent deletion: hold to confirm, type the target's name, a
  second confirmation, or **Windows Hello** (user-presence verification).
  Ordinary sensitive actions will not require it.
- A native OS-drawn prompt for critical actions, as an additional channel
  outside the WebViews.
- Persistent "remember this decision" grants (never for high risk).

### Confirmation copy

A confirmation must state **what** SERSHI wants to do, **what data** is affected,
**why** it needs permission, and **whether** the decision can be remembered.

```text
SERSHI wants to move 14 PDF files

From   Downloads
To     Documents\Invoices

This needs permission to modify files in Downloads.
☐ Always allow moving files within Documents      (not offered for high-risk actions)

[ Cancel ]                                   [ Move files ]
```

Never "Allow action?".

## Credentials

- Stored only in the OS credential store (Windows Credential Manager) behind a
  `CredentialStore` port ([ADR 0004](adr/0004-persistence.md)).
- Never in SQLite, JSON, `.env` (except git-ignored local development), logs, IPC
  payloads to the WebView, or activity.
- The UI can learn _whether_ a key is configured, never its value.

## Logging and privacy

Never logged anywhere: API keys, passwords, tokens, credentials, command text,
file contents, e-mail bodies, clipboard or screen content. Activity summaries are
fixed templates plus tool metadata. Tool error details (which may echo paths) are
kept out of activity and, in future diagnostics, redacted by default.

SERSHI sends **no analytics**. Any future telemetry must be opt-in, documented and
privacy-preserving.

## Microphone (Prompt 3) and screen (v0.2)

Details: [VOICE.md](VOICE.md).

- **Explicit.** The microphone is off at start-up. It opens only on an
  explicit push-to-talk click in the Command Center. There is no
  background listening and no wake word; `WakeWordPort` is defined but has
  no implementation.
- **Visible.** While capturing, SERSHI shows it in:
  - the mic button
  - the Listening state on the Core and the companion
  - the tray tooltip
  - Activity ("Microphone on / off" with the duration)
- **Bounded.** Capture closes on:
  - endpoint, a second click, or Escape
  - hiding the Command Center or a typed command
  - quit
  - a 30 s cap
  The session thread owns the device handle, so release is deterministic.
- **Memory only.** Audio never touches disk, logs, SQLite, localStorage or
  activity. Raw audio never crosses IPC; the UI receives a bounded 0–1 level
  and the transcript to display.
- **Local models are data.** They are downloaded only on request, from a
  pinned HTTPS URL, then size- and SHA-256-verified, atomically installed,
  and re-verified before the first load. They are never executed. No
  process is spawned.
- **Least privilege.**
  - Voice commands are granted to the Command Center only.
  - The companion only receives the level event.
  - The confirmation window's capability and bundle contain no voice code.
- Screen capture (v0.2) will be per-request and user-initiated, never
  continuous.

## Threats and mitigations

| Threat                                              | Mitigation                                                                                      |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Prompt injection from web/e-mail/screen content     | Content is data; typed tools only; `Agent` origin → confirmation for sensitive/high-risk; confirmations show concrete effects |
| Model hallucinates a destructive action             | Risk from definitions; high-risk always confirms; no shell                                      |
| Compromised WebView issues IPC calls                | Narrow commands, per-window permissions, Rust-side validation, strict CSP                       |
| Malicious or buggy third-party skill                | Declared permissions, own namespace, approval, revocation; later: signing and process isolation |
| Credential theft from disk                          | OS credential store; nothing plaintext                                                          |
| Sensitive data in logs or bug reports               | Template-only activity; redaction rules for diagnostics                                         |
| Supply-chain compromise via dependencies            | Minimal dependencies, lockfiles, restricted install scripts, `--locked` CI builds              |
| UI spoofing of confirmations                        | Pending calls live in Rust; approval only from the dedicated confirmation window, only for its assigned id, once; subject from trusted discovery |
| Compromised Command Center approves its own request | The Command Center has no approval command or confirmation data; it can only request and cancel |
| Clickjacking / inherited clicks or keys             | Cancel focused first; Approve arming delay, press-must-start-on-button, repeat keys ignored; always-on-top surface |
| Text → command injection via application names      | Names are data resolved against a discovered catalog; no shell, no arguments, path-like input rejected |
| Launching a malicious binary                        | Only discovered targets (Start Menu, App Paths, packages, built-ins) can launch; SERSHI trusts what the user or an installer already registered, like the Start Menu does |
| Elevation abuse                                     | No elevation requests; `ERROR_ELEVATION_REQUIRED` is reported, never retried elevated           |
| Data loss by closing apps                           | Sensitive + confirmation; `WM_CLOSE` only, so apps can prompt to save; no forced termination   |
| Agent Brain proposes an unwanted action (Prompt 4)  | Grammar-constrained, strictly parsed decisions; tools only from the registry manifest, apps only from offered handles; ≤ 5 steps; each step through policy; `Agent` origin; every sensitive step needs its own trusted confirmation ([AGENT_BRAIN.md](AGENT_BRAIN.md)) |
| One approval reused for a whole plan                | Approval covers one stored call; a plan's later sensitive step opens a new confirmation; decline or expiry cancels the rest |
| "Yes" / "do it" over a pending approval             | The request withdraws the approval and is understood by Gate 3C only — never through context or the model, so it cannot re-create it |
| Stale or cancelled model output acting late         | Decisions are tied to a ticket; a dismissed or replaced request's decision is discarded; plans advance only by current id |
| Commands, paths or scripts requested in chat        | Refused deterministically before any model ("I can't run commands…"); no tool can run them |
| Tampered inference engine executable                | Installer builds compile in the engine's SHA-256; a mismatching engine is never started |
| Spoken "yes" in a hands-free voice session (Gate 4.1) | The microphone is closed while a confirmation is pending; opening it withdraws the approval; no intent approves |
| Voice or the model granting itself hands-free authority | Only a closed list of low-risk permissions is configurable; loosening one waits in the trusted confirmation window; audited; high-risk tools never configurable; model-proposed sensitive steps still confirm ([ADR 0018](adr/0018-voice-session-authority.md)) |
| SERSHI's own speech heard as a command              | Half duplex: no capture during playback, plus a 350 ms echo guard |
| A late reply or plan acting after an interruption   | Barge-in abandons the brain ticket and cancels the plan before its next step; late results are discarded and never spoken |
| Wrong "what did you do?" answers                    | Answered from the action ledger (real tool results), never from the model; failures reported as failures |
| A second close approval left pending with no window (Gate 4.1.1) | Several closes are one grouped confirmation for exactly those apps (immutable, one-time); the window is destroyed only when nothing is pending; an approval whose window cannot be shown is withdrawn |
| A grouped approval widened after the fact            | The batch is stored in Rust before the window opens; approval runs only the stored calls, each target re-verified; the id works once |
| Harmless-sounding apps closed without asking          | Only a curated built-in list is "safe to close" (Calculator); everything else asks; per-app "Always allow" is approved in the trusted window; model-proposed closes always ask |

## Dependencies with a security role

| Crate       | Version | Purpose                                              | Why this one |
| ----------- | ------- | ---------------------------------------------------- | ------------ |
| `getrandom` | 0.3     | OS CSPRNG for confirmation ids (16 bytes)            | Minimal, widely audited, the primitive that `rand`/`ring` build on; already in the lockfile via Tauri, so no new code enters the build |

## Limitations

What the confirmation design does **not** defend against:

- Malware or another process running as the same Windows user (it can drive
  the UI, inject input or read process memory).
- A compromise of the SERSHI process or of the WebView2 runtime itself.
- A compromised Command Center **cancelling** pending actions or submitting
  new requests (both are safe directions; requests still pass policy and need
  approval when sensitive).
- A user deliberately approving a harmful action; SERSHI can only make the
  action and its risk clear.
- Social engineering outside SERSHI.
- Screen readers and accessibility tools that can activate buttons on the
  user's behalf, by design.

## Assumptions

- The local user account and OS are not compromised; SERSHI does not defend against
  malware already running as the user.
- The user is the only principal; multi-user and remote control are out of scope.
- Model providers may see what the user sends them; cloud use is opt-in and the UI
  will say which provider handles a request.
