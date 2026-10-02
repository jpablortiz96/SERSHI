# Memory

**Status:** session memory exists; everything else is designed for v0.7 (storage
foundations in v0.1).

SERSHI's memory must be **explicit and inspectable**. There is no invisible user
profile: the user can view, edit, delete and disable everything SERSHI remembers.

## Kinds of memory

| Kind              | Examples                                                             | Lifetime                     | Status                                   |
| ----------------- | -------------------------------------------------------------------- | ---------------------------- | ---------------------------------------- |
| Session           | The current conversation, including what SERSHI heard ("You said …") | Until SERSHI exits           | **Implemented** — UI memory only, max 40 messages, never written by the core. Audio is never kept ([VOICE.md](VOICE.md)) |
| Session context   | Recent applications, the last system fact, a few turns, an open question (Prompt 4) | Minutes; "New conversation" clears it | **Implemented** — core memory only, bounded, trusted entities only, never persisted ([AGENT_BRAIN.md](AGENT_BRAIN.md#session-context)) |
| Action ledger     | What SERSHI actually did this conversation: action, trusted application, result (Gate 4.1) | Until "New conversation" or exit | **Implemented** — core memory only, 32 entries, never persisted; the audit log is separate ([AGENT_BRAIN.md](AGENT_BRAIN.md#action-ledger-gate-41)) |
| Permission settings | Close/open applications and system information: Always allow or Ask every time (Gate 4.1) | Until changed | **Implemented** — `permissions.json` in the app config folder, closed list only, invalid file → defaults |
| Preference        | Preferred browser, editor, voice, language                           | Until changed                | v0.1 settings, v0.7 learned suggestions  |
| Operational       | "my project" → `D:\Projects\SERSHI`; "work mode" → routine id         | Until deleted                | v0.7                                     |
| Long-term         | Facts the user explicitly asks SERSHI to remember                    | Until deleted                | v0.7                                     |
| Activity          | What SERSHI did (not what the user said)                             | Session today; retention setting in v0.1 | **Implemented** (in memory)   |

## Rules

1. **Nothing is remembered silently.** Long-term and operational entries are created
   only when the user asks, or after SERSHI proposes one and the user accepts
   ("Remember that 'my project' means D:\Projects\SERSHI?").
2. **Every entry is visible** in a Memory view with its source and date, and can be
   edited or deleted individually; memory can be disabled entirely.
3. **Memory is data, not instructions.** Retrieved entries are given to a model as
   context and cannot change policy or permissions.
4. **Local by default.** Stored in the local SQLite database (with local embeddings
   via `sqlite-vec` in v0.7); sent to a cloud model only as part of a request the
   user makes with that provider selected.
5. **No secrets in memory.** Credentials go to the credential store only.

## Entry shape (v0.7)

```text
MemoryEntry
├── id
├── kind            preference | operational | longTerm
├── key / content   e.g. "my project" → "D:\Projects\SERSHI"
├── source          userStated | userConfirmedSuggestion
├── createdAt / updatedAt / lastUsedAt
└── enabled
```
