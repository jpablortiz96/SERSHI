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
        W["WebView UI"]
    end
    subgraph Trusted["Trusted (Rust)"]
        P["Policy engine + tool registry"]
        T["Typed tools"]
        OS["Operating system"]
    end
    U --> W
    W -- "narrow typed IPC" --> P
    X -. "data only" .-> M
    M -- "structured tool calls" --> P
    S3 -- "declared tools" --> P
    P --> T --> OS
```

| Boundary                 | Rule                                                                                                                                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Model → system**       | Model output is untrusted data. It can only _name_ a registered tool and supply typed input. It never becomes a shell command. Risk comes from the tool definition, not the model. |
| **External content → model** | Web pages, e-mails, documents and screen text are data. Instructions inside them never override system policy, permissions or confirmation. Content is delimited as untrusted when given to a model, and actions derived from it carry `CallOrigin::Agent`, which requires confirmation for anything sensitive. |
| **WebView → Rust (IPC)** | The UI is treated as potentially compromised (e.g. by a rendering bug or injected content). It gets narrow commands only, scoped per window; no generic execution; inputs validated in Rust. |
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
| `decide_confirmation` granted to the Command Center only | `capabilities/command-center.json`             | `commands.test.ts`                                   |
| No paths in outcomes, activity or UI (`ApplicationSummary` only) | `apps/model.rs`, `apps/tools.rs`               | `commands.test.ts` ("no paths in outcomes"), `launch_errors_are_structured_without_internal_details` |
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

The first sensitive tool, `system.close_application`, introduced the
confirmation lifecycle ([ADR 0010](adr/0010-trusted-confirmation-lifecycle.md)):

- The pending call is stored **in Rust** (`ConfirmationStore`). The UI only
  receives an opaque id plus display data, and can only answer
  `{ confirmationId, toolId, approved }`.
- Ids are 32 hex characters derived from the standard library's OS-seeded
  `RandomState` (not a CSPRNG). They are unguessable handles, not secrets: the
  real guarantees are that only the core issues them, the decision must also
  name the pending tool, and each id works once. Malformed ids are rejected
  at deserialization.
- `take()` is **one-time**: a confirmation is consumed on approval, cancel,
  expiry **and** on a mismatched tool id, so replays and guesses do nothing.
- Expiry: 90 s, enforced at decision time; at most 8 pending.
- On approval the executor **re-runs policy** (a revoked permission still wins)
  and **re-resolves** the application; if it no longer matches what the user
  approved, nothing runs (`SubjectChanged`).
- The dialog names the **resolved** application from trusted discovery data,
  never the request text or model output.
- Remembering is never offered today (`canRemember: false`); no authorization
  state lives in `localStorage`.
- A new request, dismissing the assistant or hiding the Command Center cancels
  pending confirmations. Every step is audited (`confirmationRequired`,
  `confirmationApproved`, `confirmationCancelled`, `confirmationExpired`).

The dialog is modal, focuses **Cancel** first, cancels on Escape and shows the
remaining time.

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

## Microphone and screen (v0.2–v0.3)

- No hidden capture. Listening and screen capture always show a visible,
  state-driven indicator on the companion; the Listening state exists for this.
- Push-to-talk and "wake word disabled" modes; a hardware-style mute that the
  voice pipeline cannot override.
- Wake-word detection runs locally; audio is not retained after transcription
  unless the user opts in.
- Screen capture is per-request and user-initiated, never continuous.

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
| UI spoofing of confirmations                        | Pending calls live in Rust; the UI can only approve an id the core issued, once, for the same tool; subject from trusted discovery |
| Text → command injection via application names      | Names are data resolved against a discovered catalog; no shell, no arguments, path-like input rejected |
| Launching a malicious binary                        | Only discovered targets (Start Menu, App Paths, packages, built-ins) can launch; SERSHI trusts what the user or an installer already registered, like the Start Menu does |
| Elevation abuse                                     | No elevation requests; `ERROR_ELEVATION_REQUIRED` is reported, never retried elevated           |
| Data loss by closing apps                           | Sensitive + confirmation; `WM_CLOSE` only, so apps can prompt to save; no forced termination   |

## Assumptions

- The local user account and OS are not compromised; SERSHI does not defend against
  malware already running as the user.
- The user is the only principal; multi-user and remote control are out of scope.
- Model providers may see what the user sends them; cloud use is opt-in and the UI
  will say which provider handles a request.
