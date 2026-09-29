# Architecture Decision Records

Short records of consequential decisions: the problem, the decision, the
alternatives considered and the consequences. Superseded records stay, marked as
such.

| ADR                                             | Decision                                           | Status   |
| ----------------------------------------------- | -------------------------------------------------- | -------- |
| [0001](0001-desktop-stack.md)                   | Tauri 2 + React + TypeScript + Vite                | Accepted |
| [0002](0002-process-and-module-boundaries.md)   | Three Rust crates, two TS packages, one app        | Accepted |
| [0003](0003-agent-tool-security-model.md)       | Typed tools behind a policy engine; no shell       | Accepted |
| [0004](0004-persistence.md)                     | SQLite for data, OS credential store for secrets   | Accepted (not yet implemented) |
| [0005](0005-ai-provider-abstraction.md)         | Capability-specific provider ports                 | Accepted (not yet implemented) |
| [0006](0006-windows-platform-boundary.md)       | One platform crate with a `cfg(windows)` module    | Accepted |
| [0007](0007-frontend-state-management.md)       | Rust owns state; Zustand mirrors it                | Accepted |
| [0008](0008-license.md)                         | Apache-2.0                                         | Proposed |
| [0009](0009-windows-application-discovery.md)   | Discovered apps, native launch, graceful close     | Accepted |
| [0010](0010-trusted-confirmation-lifecycle.md)  | Rust-owned, one-time, expiring confirmations       | Accepted (amended by 0011) |
| [0011](0011-dedicated-confirmation-surface.md)  | Dedicated minimal-authority confirmation window, CSPRNG ids | Accepted |
| [0012](0012-state-driven-visual-system.md)      | State-driven visual system: one table, one renderer, one motion switch | Accepted |

Template: `Status · Context · Decision · Alternatives · Consequences`.
