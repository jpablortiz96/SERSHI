# 0003 — Agent tool security model

**Status:** Accepted · 2026-09

## Context

SERSHI will let a language model propose actions on the user's computer. Model
output is untrusted: it can be wrong, and it can be steered by prompt injection
in web pages, e-mails, documents or screen content.

## Decision

1. **The only way to act is a registered, typed tool.** No shell tool, no generic
   "execute" IPC command. A tool declares id, schema, permissions, risk,
   timeout and platforms.
2. **Risk and permissions come from the registered definition**, never from the
   call. A model cannot downgrade the risk of what it proposes.
3. **Every call passes the `PolicyEngine`**, in this order: prohibited → platform →
   denied permission → high risk (always confirm, never rememberable) →
   undecided permission (confirm) → sensitive and not user-originated (confirm) →
   allow.
4. **Inputs are parsed into typed structs with `deny_unknown_fields`.**
5. **Every step is audited** in the activity log, without payloads.
6. **Calls needing confirmation are never executed** until the user approves (the
   confirmation UI arrives with the first sensitive tool).
7. **Personality and prompts cannot change policy.** Policy is code and user grants.

## Alternatives

- **Let the model write shell commands with a denylist:** denylists are always
  incomplete; rejected outright.
- **Confirm everything:** safe but unusable; users learn to click "Allow" blindly.
- **Trust levels per model/provider:** adds complexity without removing the need
  for per-action policy. Origin (`User` / `Agent` / `Routine`) is recorded and
  already tightens sensitive actions.

## Consequences

- Capabilities grow one tool at a time, each with tests for its policy behaviour.
- Arbitrary "do anything" requests are impossible by construction — intentionally.
- Third-party skills (v0.5) plug into the same registry and policy; they get their
  own permission namespace and trust metadata ([SKILLS.md](../SKILLS.md)).
