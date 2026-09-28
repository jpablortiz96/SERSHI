# 0002 — Process and module boundaries

**Status:** Accepted · 2026-09

## Context

The architecture must keep the domain portable and testable, keep Windows code
isolated, and keep the UI from reaching the OS — without an "enterprise" tree of
empty packages.

## Decision

One process (Tauri), and these units:

| Unit                     | Role                                                                 |
| ------------------------ | -------------------------------------------------------------------- |
| `sershi-core`            | Domain + application layer + port traits. No OS, no Tauri, no vendors. |
| `sershi-platform`        | Adapters implementing core ports; `windows/` for Win32 code.          |
| `sershi-desktop`         | Tauri shell: windows, IPC commands, events. No business rules.       |
| `@sershi/contracts`      | IPC types generated from Rust + runtime guards.                      |
| `@sershi/design-tokens`  | Visual tokens + runtime theming.                                     |
| `@sershi/desktop`        | Both window UIs.                                                     |

New crates are added when the compiler should enforce a boundary, e.g.:
`sershi-storage` (so only it links SQLite), `sershi-providers-*` (so vendor SDKs
stay out of core), `sershi-skills` (skill runtime and sandboxing).

## Alternatives

- **Crate per concept** (`sershi-tools`, `sershi-security`, …): more ceremony than
  enforcement today; tools, policy and executor change together.
- **Everything in the Tauri crate:** fast to start, but the domain would depend on
  Tauri and become untestable without a window.
- **Sidecar processes / microservices:** rejected. This is a desktop app; a
  sidecar only makes sense later for isolating untrusted third-party skills.

## Consequences

- `sershi-core` compiles and tests on any OS in milliseconds.
- `sershi-core` is a "fat" crate for now; split when a module needs an enforced
  boundary or it becomes hard to navigate.
- Ports exist only where a real boundary exists (`SystemInfoProvider` today),
  following the rule against single-implementation abstractions.
