# Skills

**Status:** designed, not implemented (target v0.5). This document fixes the shape
so that v0.1–v0.4 built-in capabilities can be migrated into skills without
redesign.

A skill packages related tools (Spotify, GitHub, Home Assistant, OBS, VS Code, …)
with a manifest that declares everything it may do. Skills plug into the same
`ToolRegistry` and `PolicyEngine` as built-in tools; they gain no ambient
authority.

## Manifest

```json
{
  "id": "spotify",
  "name": "Spotify",
  "version": "1.0.0",
  "sershi": ">=0.5 <1.0",
  "description": "Control Spotify playback.",
  "publisher": { "name": "Example", "url": "https://example.com" },
  "permissions": [
    { "id": "spotify.read", "reason": "See what is playing" },
    { "id": "spotify.control", "reason": "Play, pause and skip" }
  ],
  "tools": [
    { "id": "spotify.play", "risk": "safe", "permissions": ["spotify.control"], "input": "schemas/play.json" },
    { "id": "spotify.pause", "risk": "safe", "permissions": ["spotify.control"] },
    { "id": "spotify.search", "risk": "safe", "permissions": ["spotify.read"], "input": "schemas/search.json" }
  ],
  "network": ["api.spotify.com"],
  "credentials": [{ "id": "spotify.oauth", "kind": "oauth2" }]
}
```

## Rules

| Concern                  | Rule                                                                                                           |
| ------------------------ | -------------------------------------------------------------------------------------------------------------- |
| Namespaces               | A skill's tool and permission ids must start with its own id. It cannot register `system.*` or another skill's ids. |
| Permissions              | Declared up front with a human reason; approved by the user at install; changes on update require re-approval. |
| Risk                     | Declared per tool; SERSHI may raise it (e.g. anything with `files.write` is at least sensitive), never lower it. |
| Inputs                   | JSON Schema per tool, validated by SERSHI before the skill sees the call; unknown fields rejected.              |
| Network                  | Declared host allow-list; no undeclared outbound connections (enforced once skills run out of process).        |
| Credentials              | Requested by id; stored in the OS credential store; the skill receives a scoped handle, never raw storage access. |
| Isolation                | Phase 1: declarative skills (HTTP + manifest) run by SERSHI. Phase 2: code skills in a separate process or WASM sandbox with only the granted capabilities. |
| Trust                    | Trust metadata: built-in, verified publisher (signed), local/unsigned (extra warning).                         |
| Compatibility            | `sershi` SemVer range; incompatible skills are disabled, not loaded.                                           |
| Lifecycle                | Install, enable/disable, update, uninstall (removes grants and credentials).                                  |
| Audit                    | Every skill tool call appears in Activity with the skill's name.                                              |

## Out of scope

Marketplace, payments, remote installation, automatic updates without approval.
