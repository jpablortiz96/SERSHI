# 0007 — Frontend state management

**Status:** Accepted · 2026-09

## Context

Two windows (separate JavaScript contexts) must always show the same assistant
state. The UI must not grow a second, divergent model of that state.

## Decision

- **Rust owns shared state.** The assistant state machine and activity log live in
  `sershi-core`; changes are broadcast as events to every window.
- **Zustand stores mirror** the latest snapshot (newest `revision` wins) and hold
  session-only UI state (conversation transcript, local previews).
- **No server-cache library** (TanStack Query) yet: there are three reads, two of
  which are pushed by events. Revisit when remote data (calendars, e-mail) arrives.
- **No animation library** yet: CSS keyframes and transitions on
  `transform`/`opacity` cover Prompt 0. Motion/Rive are candidates for companion
  packs (v0.8).

## Alternatives

- **React context + `useSyncExternalStore`:** viable and dependency-free, but
  Zustand (~1 KB, zero dependencies) gives selectors and devtools with less code.
- **Redux Toolkit:** far more ceremony than two small stores need.
- **State in the frontend, persisted via IPC:** would make the WebView the source
  of truth for something the Rust policy engine must trust. Rejected.

## Consequences

- Adding a window never requires state synchronisation code.
- Stores stay small; if one grows business logic, that logic belongs in Rust.
