# 0008 — License: Apache-2.0

**Status:** Proposed — maintainers should confirm before the first public release · 2026-09

## Context

SERSHI is open source, aims for an ecosystem of third-party skills, and may be
used and redistributed by companies.

## Decision

License the project under **Apache License 2.0** (`LICENSE`).

## Alternatives

| License    | Notes                                                                                           |
| ---------- | ----------------------------------------------------------------------------------------------- |
| MIT        | Simplest and most permissive, but no explicit patent grant or patent-retaliation clause.         |
| Apache-2.0 | Permissive, explicit patent grant and retaliation, NOTICE handling; the norm for Rust/Tauri projects. |
| MPL-2.0    | File-level copyleft; keeps modifications to SERSHI files open while allowing proprietary skills. |
| GPL-3.0    | Strong copyleft; discourages commercial adoption and complicates proprietary skills.            |

## Consequences

- Contributors license contributions under Apache-2.0 (inbound = outbound; see
  CONTRIBUTING.md). No CLA for now.
- Third-party skills may use any license compatible with dynamic plug-in use.
- Changing the license later requires agreement from all contributors, so decide
  before accepting significant outside contributions.
