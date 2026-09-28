# 0006 — Windows platform boundary

**Status:** Accepted · 2026-09

## Context

SERSHI is Windows-first but is developed and tested largely on Linux (cloud
environments, CI). Windows-only behaviour must never be claimed as validated
because Linux tests pass, and Win32 code must not spread through the codebase.

## Decision

- All OS access goes through **ports** in `sershi-core`.
- Adapters live in **`sershi-platform`**:
  - portable adapters (e.g. `sysinfo`-backed) in the crate root;
  - Windows-only integrations in `src/windows/`, compiled only with
    `#[cfg(windows)]`, the single place allowed to call Win32/WinRT.
- Window behaviour (transparency, always-on-top, placement) stays in the Tauri
  shell's `surfaces.rs`.
- **Capabilities are reported honestly** at runtime (`available`,
  `requiresWindowsValidation`, `planned`, `unsupported`); on Windows nothing is
  `available` until it passes the manual checklist in
  [WINDOWS_PLATFORM.md](../WINDOWS_PLATFORM.md). A unit test enforces this.
- A `windows-latest` CI job compiles the Windows module and runs all tests.

## Alternatives

- **Separate `sershi-platform-windows` crate:** cleaner once Windows code is
  substantial; premature while the module is empty. Revisit when it exceeds a few
  hundred lines or pulls in the `windows` crate.
- **Conditional code inline where needed:** rejected; it scatters platform logic.

## Consequences

- Linux remains a development platform, not a product platform.
- Every Windows behaviour carries **REQUIRES_WINDOWS_VALIDATION** until verified on
  hardware, and the doc checklist is the record of validation.
