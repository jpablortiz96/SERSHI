# 0010 — Trusted confirmation lifecycle

**Status:** Accepted · 2026-09

## Context

Sensitive actions (first: closing an application) need the user's approval.
The approval must not be forgeable by model output, tool output, translated
strings or a manipulated WebView, and must not be replayable.

## Decision

- When policy requires approval, the executor returns a **draft** and nothing
  runs. The service stores the exact `ToolCall` in a `ConfirmationStore` under
  an unguessable one-time id (32 hex characters from the OS-seeded
  `RandomState`; not a CSPRNG, and not relied on as a secret) with a
  **90-second** expiry.
- Before asking, the tool's `prepare` resolves the **target** from trusted
  data (e.g. the discovered application). The UI receives only structured
  fields — action, subject summary, risk, reason, expiry — and renders them
  with its own localized copy.
- The UI can send only `{ confirmationId, toolId, approved }`. Rust checks the
  id is known and pending (one-time: any decision, mismatch or expiry consumes
  it), unexpired, and for the same tool; then re-evaluates policy and
  re-resolves the target, refusing if it changed.
- A new request, dismissing the assistant or hiding the Command Center
  cancels pending confirmations. An expiry timer returns SERSHI to idle.
- New assistant state **`AwaitingConfirmation`** (not transient, cannot sleep,
  only an approval leads to `Executing`). `Warning` remains the transient
  "nothing happened" outcome; reusing it would auto-settle after 4 s while the
  dialog is still open.
- "Remember this decision" is never offered yet (no persistent grants until
  the v0.1 settings store; never for high risk). No authorization state lives
  in `localStorage`.

## Alternatives

- **Frontend holds the call and re-submits it on approval:** the WebView could
  alter the call; rejected.
- **Sign the call and hand it to the UI:** more machinery, same outcome as
  keeping it in Rust.
- **Reuse `Warning`:** semantically wrong and visibly inconsistent (settles
  while waiting).

## Consequences

- The first sensitive tool ships with a complete, tested approval path:
  unconfirmed, denied, expired, replayed, forged and mismatched decisions never
  execute (`crates/sershi-core/src/service/tests.rs`).
- Future sensitive tools only implement `prepare` and pick a
  `ConfirmationAction` to get the same guarantees.
