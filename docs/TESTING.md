# Testing

A pragmatic pyramid: most confidence comes from fast, deterministic tests of the
portable core; a few tests guard the contracts between layers; Windows behaviour
is validated manually until it can be automated.

| Layer                 | What                                                                                 | Where                                           | Command                     |
| --------------------- | ------------------------------------------------------------------------------------ | ----------------------------------------------- | --------------------------- |
| Unit (Rust)           | State transitions, policy decisions, registry, ids, intent (EN/ES/PT open/close, statements never act), activity, formatting, application catalog (de-duplication, exact/alias/prefix/word resolution, ambiguity, not-found), catalog manager (lazy scan, stale re-scan) | `crates/sershi-core/src/**` `#[cfg(test)]`      | `cargo test -p sershi-core` |
| Security (Rust)       | Denied/unconfirmed tools never execute, high-risk always confirms, hostile ids rejected, no user text or error details in activity, unknown input fields rejected; application tools reject paths/commands/extra fields and never launch on ambiguity; confirmations: unconfirmed, cancelled, expired, replayed, forged, malformed and stale-surface decisions never execute; decisions with any field beyond `{ confirmationId, decision }` are rejected; the executed action is the stored call; typing "yes" never approves; activity never contains the id or input; CSPRNG ids are 128-bit, valid hex and distinct; surface assignment (one surface, stale surface cannot decide, close releases only its own); approval re-runs policy and refuses a changed subject | `policy.rs`, `executor.rs`, `service/tests.rs`, `confirmation.rs`, `apps/tools.rs`, `ids.rs`, `tool.rs` | `cargo test -p sershi-core` |
| Windows compile (Rust)| The `cfg(windows)` adapter and the Tauri shell (tray, shortcut, single instance) type-check and pass clippy | Windows CI job; locally `cargo clippy --workspace --target x86_64-pc-windows-msvc -- -D warnings` | — |
| Integration (Rust)    | `sysinfo` adapter against the real host OS; capability honesty                      | `crates/sershi-platform`                        | `cargo test -p sershi-platform` |
| Contract              | Rust-serialized fixtures satisfy TS guards; TS and Rust command lists match; generated types have no drift | `packages/contracts/test`, CI drift step | `pnpm test`, `pnpm contracts:generate` |
| IPC security          | Capability files grant only declared commands; companion allow-list; only the confirmation window can decide confirmations (Command Center and companion cannot); the confirmation window has exactly two permissions; no wildcard window labels; no generic execution commands; exact risk/permission map per tool; application tools accept only `application`; no paths in outcomes | `packages/contracts/test/commands.test.ts` | `pnpm test` |
| Design tokens         | Variable naming, state coverage, motion budget, WCAG AA contrast, theme overrides    | `packages/design-tokens/test`                   | `pnpm test`                 |
| Localization          | Every locale has every key, matching placeholders, no empty or copied-English messages, labels for every state; OS mapping and fallback; manual override and Automatic; persistence; live switching without restart; cross-window sync; unsupported locales never crash; `Intl` formatting; outcomes phrased per locale | `apps/desktop/test/i18n.test.ts`, `locale-switch.test.tsx` | `pnpm test` |
| Copy guard            | No hard-coded user-facing strings (JSX text, `aria-label`, `placeholder`, `title`, `data-tip`) in components and surfaces | `apps/desktop/test/hardcoded-strings.test.ts` | `pnpm test` |
| UI                    | Core renders every state accessibly; command input, recall, honest offline replies; store revision ordering; confirmation surface (Cancel focused, Escape and × cancel, arming delay, a press that began before arming or a repeated key cannot approve, blur disarms, sends only id + choice, expiry sends nothing, localized in three languages); the Command Center facade has no confirmation methods, only the surface imports its IPC module, and a pending approval renders with no approve button; application replies and candidate buttons in three languages | `apps/desktop/test` (`confirmation-surface.test.tsx`, `applications.test.tsx`, …) | `pnpm test` |
| Visual system         | State → visual table covers every state; thinking / planning / executing differ in motion and shape; attention only for outcomes and approvals; Core variants share one renderer; decorative cores are hidden from assistive tech; every looping animation is neutralised under `html[data-motion="reduced"]` and modules use no private reduced-motion media queries; contextual presence (calm / normal / present) | `apps/desktop/test/visual-system.test.tsx` | `pnpm test` |
| Appearance            | Motion and companion-size preferences parse safely, persist without losing the language, drive `<html data-motion>`, follow the OS and other windows, and are real localized settings | `apps/desktop/test/appearance.test.tsx` | `pnpm test` |
| Companion & transitions | Click summons, drag never summons and settles, presence follows the Command Center's visibility, size follows the setting, the companion shows no approval control; command acceptance sends once and clears; window exit plays then hides (immediately with reduced motion); one entrance for every summon origin | `apps/desktop/test/presence.test.tsx` | `pnpm test` |
| Gate 1A invariants    | Confirmation window exactly two commands; Command Center and companion cannot decide or read confirmations; companion permissions pinned exactly; presence is an event, not a command; no window-creating or generic-execution commands or permissions | `packages/contracts/test/commands.test.ts` | `pnpm test` |
| Manual (Windows)      | Gate 2A: application operation, trusted confirmations, tray, shortcut, single instance, companion, visual states, transitions, DPI, languages, idle CPU | [WINDOWS_PLATFORM.md](WINDOWS_PLATFORM.md#manual-validation-checklist) | — |
| E2E                   | Deferred until v0.1 flows stabilise; only for high-value flows (command → tool → UI) | —                                               | —                           |

## Commands

```bash
pnpm check        # format, lint, typecheck, TS tests, frontend build
pnpm check:rust   # cargo fmt --check, clippy -D warnings, cargo test
pnpm contracts:generate   # regenerate TS types + fixtures from Rust
```

## Conventions

- Test behaviour, not implementation details; name tests after the rule they
  protect (`high_risk_always_confirms_and_cannot_be_remembered`).
- Every new tool ships with policy tests for its risk and permission behaviour.
- Test fakes live next to the code (`test_support` modules); no mocking framework.
- No arbitrary coverage target. A change that alters security behaviour without a
  test change is a review blocker.
- No GUI tests in CI yet; they would be flaky without a real window manager.

## Visual checks in the cloud

The UI can be exercised without Windows:

- `pnpm dev:web` — browser preview (no Rust core); `?preview=<state>` renders any
  state, e.g. `http://localhost:1420/companion.html?preview=thinking`.
- `pnpm dev` under `Xvfb` runs the real Tauri app on Linux (WebKitGTK). Window
  manager features (always-on-top, transparency compositing) are not observable
  there — see [WINDOWS_PLATFORM.md](WINDOWS_PLATFORM.md).
