# 0009 — Windows application discovery and launch

**Status:** Accepted · 2026-09

## Context

SERSHI must open installed applications by name ("Abre Spotify"), including
Microsoft Store apps that have no stable `.exe` path, without ever turning
user or model text into a command, and without slowing start-up.

## Decision

- Applications are **discovered**, not guessed: built-ins, packaged apps from
  `shell:AppsFolder` (AUMIDs), Start Menu shortcuts read with `IShellLinkW`,
  and `App Paths`. Results are merged by priority into an in-memory catalog.
- Callers supply only a **name** (validated, ≤ 80 chars, no path characters),
  resolved in deterministic tiers; ambiguity asks instead of guessing.
- Launch per target kind: `CreateProcessW` for executables (no shell; no
  implicit UAC), `IApplicationActivationManager` for packaged apps,
  `ShellExecuteExW` only for fixed SERSHI-defined URIs and installer-managed
  shortcuts.
- Close sends `WM_CLOSE` to matching top-level windows; never terminates.
- Catalog: lazy/deferred scan, manual refresh, one stale re-scan on a miss; no
  polling; native calls bounded by timeouts.
- `unsafe` is allowed only in `crates/sershi-platform/src/windows` (workspace
  lint changed from `forbid` to `deny` with a single, documented opt-out).

## Alternatives

- **`cmd /c start <name>` or PowerShell `Start-Process`:** trivially injectable
  and slow; rejected.
- **`ShellExecute` for everything:** simplest, but triggers UAC prompts
  implicitly and executes whatever a `.lnk` points to without inspection.
- **Crawling Program Files / PATH:** slow, noisy (helpers, updaters), and finds
  things that aren't user-facing applications.
- **Fuzzy (edit-distance) matching:** convenient, but can open the wrong
  program; rejected in favour of explicit ambiguity.

## Consequences

- Store apps work without paths; paths never reach the UI.
- Some apps can't be closed safely (UWP frame hosts, tray-only apps); SERSHI
  says so.
- Discovery quality depends on well-formed Start Menu entries; unusual
  installs may be missing (documented in APPLICATIONS.md).
