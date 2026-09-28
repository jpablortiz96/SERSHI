# Testing

A pragmatic pyramid: most confidence comes from fast, deterministic tests of the
portable core; a few tests guard the contracts between layers; Windows behaviour
is validated manually until it can be automated.

| Layer                 | What                                                                                 | Where                                           | Command                     |
| --------------------- | ------------------------------------------------------------------------------------ | ----------------------------------------------- | --------------------------- |
| Unit (Rust)           | State transitions, policy decisions, registry, ids, intent, activity, formatting     | `crates/sershi-core/src/**` `#[cfg(test)]`      | `cargo test -p sershi-core` |
| Security (Rust)       | Denied/unconfirmed tools never execute, high-risk always confirms, hostile ids rejected, no user text or error details in activity, unknown input fields rejected | `policy.rs`, `executor.rs`, `service.rs`, `ids.rs`, `tool.rs` | `cargo test -p sershi-core` |
| Integration (Rust)    | `sysinfo` adapter against the real host OS; capability honesty                      | `crates/sershi-platform`                        | `cargo test -p sershi-platform` |
| Contract              | Rust-serialized fixtures satisfy TS guards; TS and Rust command lists match; generated types have no drift | `packages/contracts/test`, CI drift step | `pnpm test`, `pnpm contracts:generate` |
| IPC security          | Capability files grant only declared commands; companion allow-list; no generic execution commands | `packages/contracts/test/commands.test.ts` | `pnpm test` |
| Design tokens         | Variable naming, state coverage, motion budget, WCAG AA contrast, theme overrides    | `packages/design-tokens/test`                   | `pnpm test`                 |
| UI                    | Core renders every state accessibly; command input, recall, honest offline replies; store revision ordering | `apps/desktop/test` | `pnpm test` |
| Manual (Windows)      | Window behaviour, transparency, DPI, installer                                       | [WINDOWS_PLATFORM.md](WINDOWS_PLATFORM.md#manual-validation-checklist) | — |
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
