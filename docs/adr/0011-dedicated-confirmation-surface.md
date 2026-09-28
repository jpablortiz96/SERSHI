# 0011 — Dedicated confirmation surface and CSPRNG ids

**Status:** Accepted · 2026-09 · amends [0010](0010-trusted-confirmation-lifecycle.md)

## Context

ADR 0010 kept the pending action in Rust, but the Command Center WebView both
rendered the dialog and held the authority to approve it
(`decide_confirmation`, `get_pending_confirmation`). SECURITY.md treats every
WebView as semi-trusted. If the Command Center were compromised (an XSS bug, a
dependency, future rich or external content, a plugin), the attacker would
inherit the ability to approve whatever SERSHI had asked the user to confirm.
The id was also derived from `RandomState`, which is not a cryptographic RNG.

## Decision

- **A separate window is the only approval surface.** Label `confirmation`,
  entry point `confirmation.html` (its own small bundle: no Command Center
  code), created and destroyed by the Rust shell only
  (`src-tauri/src/confirmation.rs`). The frontend has no command that creates
  windows.
- **Minimum capability.** `capabilities/confirmation.json` grants exactly
  `get_confirmation_context` and `decide_confirmation` — no events, window
  control, telemetry, activity, catalog, settings or other commands. The
  Command Center loses both confirmation commands; the companion never had
  them. Both commands also check the calling window's label in Rust.
- **Assignment.** The shell tracks which confirmation the surface was opened
  for (`SurfaceAssignment`, tested in the core). The surface can read and
  decide only that one; a stale surface cannot decide a replacement.
- **Decision contract:** `{ confirmationId, decision: "approve" | "cancel" }`
  with `deny_unknown_fields`. No `toolId`: the id selects the stored action,
  and everything else (tool, input, target, risk, permission) comes from it.
  An enum rather than a boolean leaves room for stronger future decisions.
- **Ids from the OS CSPRNG:** 16 bytes from `getrandom` 0.3 (`ProcessPrng` on
  Windows, `getrandom(2)` on Linux), as 32 lowercase hex characters. If the OS
  cannot provide randomness, no confirmation is created (fail closed).
- **One pending confirmation.** A new request cancels the previous one
  (nothing runs). TTL shortened from 90 s to **60 s**.
- **Lifecycle.** The surface opens when a confirmation is created, reloads
  (re-arming its input guard) if the confirmation is replaced, and is
  destroyed on approval, cancel, expiry, dismissal, hiding the Command Center
  and quit. Closing it (× or the OS close request) is a cancel. The outcome is
  sent to the Command Center as a `sershi://command-outcome` event; the
  Command Center only ever learns that approval is pending.
- **Accidental input.** Cancel has initial focus; Escape cancels; Approve is
  inert for 600 ms after the window is focused and accepts only a press that
  began on it after arming, or a fresh (non-repeated) key press.
- **Hardening of the window.** Navigation limited to the app's own page,
  devtools off in release builds, always on top, no external or model-authored
  content, same strict CSP.
- **Levels.** Requests carry `level: "standard" | "highRisk"`. A future
  `critical` level (hold to confirm, type the target's name, Windows Hello) is
  documented, not implemented.

## Alternatives

- **Keep the dialog in the Command Center, harden the WebView:** the approval
  channel would still live in the largest, most exposed renderer.
- **Native OS message box (`MessageBoxW`):** strong isolation from the
  WebViews but no SERSHI design, poor localization control, blocks a thread,
  and no path to richer future confirmation modes. Could complement the
  surface for a future `critical` level.
- **Per-request window labels (`confirmation-<n>`):** capability files would
  need wildcards, which widen the grant; one stable label plus an assignment
  in Rust gives the same one-surface-per-confirmation guarantee.
- **`rand` / `ring`:** larger dependencies for 16 random bytes; `getrandom` is
  what they use underneath and is already in the lockfile via Tauri.

## Consequences

- Compromising the Command Center no longer grants approval. It can still
  request actions and cancel pending ones (safe direction) — see
  SECURITY.md, "Limitations".
- A separate WebView is **not** an enclave: same process, same origin, same
  WebView2 runtime. The gain is a much smaller authority surface and no
  untrusted content in the one place that can approve.
- Pending confirmations live only in memory: quitting or a crash discards
  them, and nothing about them is ever persisted.
