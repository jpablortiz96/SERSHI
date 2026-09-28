# 0004 — Persistence: SQLite for data, OS credential store for secrets

**Status:** Accepted, not yet implemented (target: v0.1) · 2026-09

## Context

SERSHI needs to persist settings, permission grants, activity history, routines,
installed skills, conversation metadata and memory entries. It also needs API
keys and OAuth tokens for optional cloud providers.

## Decision

- **SQLite** via `rusqlite` with the `bundled` feature (no system dependency),
  owned by a new `sershi-storage` crate that implements core ports
  (`GrantStore`, `ActivityStore`, …). Schema migrations are versioned SQL files
  applied at start-up. WAL mode. Database in `%APPDATA%\SERSHI\sershi.db`.
- **Secrets never touch SQLite or files.** A `CredentialStore` port with a Windows
  Credential Manager adapter (`keyring` crate or direct `CredWriteW`), reviewed
  for REQUIRES_WINDOWS_VALIDATION.
- **Development `.env` files** are git-ignored and never read in release builds.

## Alternatives

- **JSON files:** no transactions or queries; corrupt on crash.
- **sled / redb:** fine key-value stores, but activity/memory need queries and
  users benefit from a standard, inspectable format.
- **Tauri store plugin:** convenient but frontend-accessible; storage must stay
  behind the Rust boundary.

## Consequences

- Until v0.1 storage lands, grants are defaults and activity is in-memory (the UI
  says so).
- Memory "view / edit / delete" becomes straightforward SQL, which supports the
  inspectability promise in [MEMORY.md](../MEMORY.md).
- Embeddings (v0.7) can use `sqlite-vec` in the same file, avoiding a separate
  vector database.
